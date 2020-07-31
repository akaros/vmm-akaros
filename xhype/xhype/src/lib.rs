/* SPDX-License-Identifier: GPL-2.0-only */
pub mod apic;
pub mod bios;
pub mod consts;
pub mod cpuid;
pub mod decode;
pub mod err;
pub mod hv;
pub mod ioapic;
pub mod linux;
pub mod mach;
pub mod pci;
pub mod rtc;
pub mod serial;
pub mod utils;
pub mod virtio;
pub mod vmexit;
pub mod vthread;

use apic::Apic;
use consts::x86::*;
use consts::*;
use crossbeam_channel::unbounded as channel;
use crossbeam_channel::{Receiver, Sender};
use err::Error;
use hv::ffi::{HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE};
use hv::vmx::*;
use hv::{MemSpace, X86Reg, DEFAULT_MEM_SPACE, VCPU};
use ioapic::IoApic;
#[allow(unused_imports)]
use log::*;
use mach::{vm_self_region, MachVMBlock};
use pci::PciBus;
use rtc::Rtc;
use serial::Serial;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use virtio::{mmio::VirtioMmioDev, VirtioDevice};
use vmexit::*;
////////////////////////////////////////////////////////////////////////////////
// VMManager
////////////////////////////////////////////////////////////////////////////////

/// VMManager is the interface to create VMs
/// only one VMManager can exist at one time!
pub struct VMManager {}

impl VMManager {
    pub fn new() -> Result<Self, Error> {
        hv::vm_create(0)?;
        Ok(VMManager {})
    }

    pub fn create_vm(
        &self,
        cores: u32,
        low_mem_size: Option<usize>,
    ) -> Result<VirtualMachine, Error> {
        assert_eq!(cores, 1); //FIXME: currently only one core is supported
        VirtualMachine::new(&self, cores, low_mem_size)
    }
}

impl Drop for VMManager {
    fn drop(&mut self) {
        hv::vm_destroy().unwrap();
    }
}

////////////////////////////////////////////////////////////////////////////////
// VirtualMachine
////////////////////////////////////////////////////////////////////////////////

type AddressConverter = Arc<Box<dyn Fn(u64) -> usize + Send + Sync + 'static>>;

#[derive(Debug, Copy, Clone)]
pub enum PortPolicy {
    AllOne,
    Random,
}

#[derive(Debug, Copy, Clone)]
pub enum MsrPolicy {
    Random,
    AllOne,
    GP,
}

#[derive(Debug)]
pub enum PolicyList<T> {
    Apply(HashSet<T>),
    Except(HashSet<T>),
}

/// A VirtualMachine is the physical hardware seen by a guest, including physical
/// memory, number of cpu cores, etc.
pub struct VirtualMachine {
    pub(crate) mem_space: RwLock<MemSpace>,
    cores: u32,
    pub(crate) low_mem: Option<RwLock<MachVMBlock>>, // memory below 4GiB
    pub(crate) high_mem: RwLock<Vec<MachVMBlock>>,
    pub(crate) virtio_mmio_devices: Vec<Mutex<VirtioMmioDev>>,
    // serial ports
    pub(crate) com1: Mutex<Serial>,
    pub(crate) ioapic: Arc<RwLock<IoApic>>,
    pub(crate) vector_senders: Arc<Mutex<Option<Vec<Sender<u8>>>>>,
    pub(crate) vcpu_ids: Arc<RwLock<Vec<u32>>>,
    pub(crate) rtc: RwLock<Rtc>,
    pub pci_bus: Mutex<PciBus>,
    pub port_list: PolicyList<u16>,
    pub port_policy: PortPolicy,
    pub msr_list: PolicyList<u32>,
    pub msr_policy: MsrPolicy,
    pub gpa2hva: AddressConverter,
    pub irq_sender: Sender<u32>,
    pub virtio_base: usize,
}

impl VirtualMachine {
    // make it private to force user to create a vm by calling create_vm to make
    // sure that hv_vm_create() is called before hv_vm_space_create() is called
    fn new(_vmm: &VMManager, cores: u32, low_mem_size: Option<usize>) -> Result<Self, Error> {
        let ioapic = Arc::new(RwLock::new(IoApic::new()));
        let vector_senders = Arc::new(Mutex::new(None));
        let (irq_sender, irq_receiver) = channel::<u32>();
        let vcpu_ids = Arc::new(RwLock::new(vec![u32::MAX; cores as usize]));
        let mut mem_space = MemSpace::create()?;
        Self::map_host_mem(&mut mem_space)?;
        let virtio_base;
        let (low_mem, gpa2hva): (_, AddressConverter) = if let Some(size) = low_mem_size {
            let low_mem_block = MachVMBlock::new(size)?;
            mem_space.map(
                low_mem_block.start,
                0,
                size,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
            )?;
            let low_mem_start = low_mem_block.start;
            let converter = move |gpa: u64| {
                if gpa < size as u64 {
                    gpa as usize + low_mem_start
                } else {
                    gpa as usize
                }
            };
            virtio_base = std::cmp::max(GiB, size);
            (
                Some(RwLock::new(low_mem_block)),
                Arc::new(Box::new(converter)),
            )
        } else {
            virtio_base = GiB; // just use the 1 GiB address as virtio base
            let converter = |gpa: u64| gpa as usize;
            (None, Arc::new(Box::new(converter)))
        };
        let vm = VirtualMachine {
            mem_space: RwLock::new(mem_space),
            cores,
            com1: Mutex::new(Serial::new(4, irq_sender.clone())),
            pci_bus: Mutex::new(PciBus::new()),
            ioapic: ioapic.clone(),
            vcpu_ids: vcpu_ids.clone(),
            rtc: RwLock::new(Rtc { reg: 0 }),
            vector_senders: vector_senders.clone(),
            port_list: PolicyList::Except(HashSet::new()),
            port_policy: PortPolicy::AllOne,
            msr_list: PolicyList::Except(HashSet::new()),
            msr_policy: MsrPolicy::GP,
            virtio_mmio_devices: Vec::new(),
            gpa2hva,
            irq_sender: irq_sender,
            low_mem,
            high_mem: RwLock::new(vec![]),
            virtio_base,
        };
        // start a thread for IO APIC to collect interrupts
        std::thread::Builder::new()
            .name("IO APIC".into())
            .spawn(move || IoApic::dispatch(vector_senders, irq_receiver, ioapic, vcpu_ids))
            .expect("cannot create a thread for IO APIC");
        Ok(vm)
    }

    // setup the identity mapping from guest physical address to host virtual
    // address.
    fn map_host_mem(mem_space: &mut MemSpace) -> Result<(), Error> {
        let mut trial_addr = 1;
        loop {
            match vm_self_region(trial_addr) {
                Ok((start, size, info)) => {
                    if info.protection > 0 {
                        mem_space.map(start, start, size, info.protection as u64)?;
                    }
                    trial_addr = start + size;
                }
                Err(_) => {
                    break;
                }
            }
        }
        Ok(())
    }

    pub unsafe fn read_guest_mem<T>(&self, gpa: u64, index: u64) -> &T {
        let hva = (self.gpa2hva)(gpa);
        let ptr = (hva + index as usize * std::mem::size_of::<T>()) as *const T;
        &*ptr
    }

    pub fn add_virtio_mmio_device(&mut self, dev: VirtioDevice) {
        let mmio_dev = VirtioMmioDev {
            dev,
            addr: self.virtio_base + self.virtio_mmio_devices.len() * PAGE_SIZE,
        };
        self.virtio_mmio_devices.push(Mutex::new(mmio_dev));
    }
}

////////////////////////////////////////////////////////////////////////////////
// GuestThread
////////////////////////////////////////////////////////////////////////////////

pub struct GuestThread {
    pub vm: Arc<VirtualMachine>,
    pub id: u32,
    pub init_vmcs: HashMap<u32, u64>,
    pub init_regs: HashMap<X86Reg, u64>,
    pub pat_msr: u64,
    pub apic: Apic,
    pub vector_receiver: Option<Receiver<u8>>,
}

impl GuestThread {
    pub fn new(vm: &Arc<VirtualMachine>, id: u32) -> Self {
        GuestThread {
            vm: Arc::clone(vm),
            id: id,
            init_vmcs: HashMap::new(),
            init_regs: HashMap::new(),
            pat_msr: 0,
            // assume that apic id = cpu id, and cpu 0 is BSP
            apic: Apic::new(APIC_BASE as u64, true, false, id, id == 0),
            vector_receiver: None,
        }
    }

    pub fn start(mut self) -> std::thread::JoinHandle<Result<(), Error>> {
        std::thread::spawn(move || {
            let vcpu = VCPU::create()?;
            {
                self.vm.vcpu_ids.write().unwrap()[self.id as usize] = vcpu.id();
            }
            self.run_on(&vcpu)
        })
    }

    fn run_on(&mut self, vcpu: &VCPU) -> Result<(), Error> {
        vcpu.set_space(&self.vm.mem_space.read().unwrap())?;
        let result = self.run_on_inner(vcpu);
        vcpu.set_space(&DEFAULT_MEM_SPACE)?;
        result
    }

    fn run_on_inner(&mut self, vcpu: &VCPU) -> Result<(), Error> {
        vcpu.enable_msrs()?;
        for (field, value) in self.init_vmcs.iter() {
            vcpu.write_vmcs(*field, *value)?;
        }
        for (reg, value) in self.init_regs.iter() {
            vcpu.write_reg(*reg, *value)?;
        }
        let mut result: HandleResult;
        let mut last_ept_gpa = 0;
        let mut ept_count = 0;
        loop {
            if let Some(deadline) = self.apic.next_timer {
                vcpu.run_until(deadline)?;
            } else {
                vcpu.run_until(u64::MAX)?;
            }
            let reason = vcpu.read_vmcs(VMCS_RO_EXIT_REASON)?;
            let rip = vcpu.read_reg(X86Reg::RIP)?;
            let instr_len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
            trace!(
                "vm exit reason = {}, rip = {:x}, len = {}",
                reason,
                rip,
                instr_len
            );
            result = match reason {
                VMX_REASON_IRQ => {
                    /*
                    VMX_REASON_IRQ happens when IO APIC calls interrupt_vcpu(),
                    see IoApic::dispatch(). But there are cases where this thread
                    is not running vm, instead, its handling vm exits. In this
                    case interrupt_vcpu() has no effects. Therefore we have
                    to check IO APIC at the end of handling each vm exit,
                    instead of only VMX_REASON_IRQ.
                    */
                    HandleResult::Resume
                }
                VMX_REASON_CPUID => handle_cpuid(&vcpu, self)?,
                VMX_REASON_HLT => HandleResult::Exit,
                VMX_REASON_MOV_CR => handle_cr(vcpu, self)?,
                VMX_REASON_RDMSR => handle_msr_access(vcpu, self, true)?,
                VMX_REASON_WRMSR => handle_msr_access(vcpu, self, false)?,
                VMX_REASON_IO => handle_io(vcpu, self)?,
                VMX_REASON_IRQ_WND => {
                    debug_assert_eq!(vcpu.read_reg(X86Reg::RFLAGS)? & FL_IF, FL_IF);
                    let mut ctrl_cpu = vcpu.read_vmcs(VMCS_CTRL_CPU_BASED)?;
                    ctrl_cpu &= !CPU_BASED_IRQ_WND;
                    vcpu.write_vmcs(VMCS_CTRL_CPU_BASED, ctrl_cpu)?;
                    HandleResult::Resume
                }
                VMX_REASON_VMX_TIMER_EXPIRED => {
                    debug_assert!(self.apic.next_timer.is_some());
                    self.apic.fire_timer_interrupt(vcpu);
                    HandleResult::Resume
                }
                VMX_REASON_EPT_VIOLATION => {
                    let ept_gpa = vcpu.read_vmcs(VMCS_GUEST_PHYSICAL_ADDRESS)?;
                    let ret = handle_ept_violation(vcpu, self, ept_gpa as usize)?;
                    if cfg!(debug_assertions) && ret == HandleResult::Resume {
                        if ept_gpa == last_ept_gpa {
                            ept_count += 1;
                        } else {
                            ept_count = 0;
                            last_ept_gpa = ept_gpa;
                        }
                        if ept_count > 10 {
                            let err_msg = format!(
                                "EPT violation at {:x} for {} times",
                                last_ept_gpa, ept_count
                            );
                            error!("{}", &err_msg);
                            return Err((reason, err_msg))?;
                        }
                    }
                    ret
                }
                _ => {
                    let err_msg =
                        format!("handler for vm exit code 0x{:x} is not implemented", reason);
                    error!("{}", err_msg);
                    Err((reason, err_msg))?
                }
            };
            match result {
                HandleResult::Exit => break,
                HandleResult::Next => vcpu.write_reg(X86Reg::RIP, rip + instr_len)?,
                HandleResult::Resume => (),
            }
            if result == HandleResult::Next {
                /*
                sti and mov-ss block interrupts until the end of the next instruction
                Since the handle result is next, if there is existing sti/mov-ss
                blocking, we should clear it.
                */
                let mut irq_ignore = vcpu.read_vmcs(VMCS_GUEST_IGNORE_IRQ)?;
                if irq_ignore & 0b11 != 0 {
                    irq_ignore &= !0b11;
                    vcpu.write_vmcs(VMCS_GUEST_IGNORE_IRQ, irq_ignore)?;
                }
            }
            // collect interrupt vector from IO APIC
            if let Some(ref receiver) = self.vector_receiver {
                if let Ok(vector) = receiver.try_recv() {
                    self.apic.fire_external_interrupt(vector);
                }
            }
            self.apic.inject_interrupt(vcpu)?;
        }
        Ok(())
    }
}

extern "C" {
    pub fn hlt();
}

#[cfg(test)]
mod test {
    use super::VMManager;
    #[test]
    fn create_vm_test() {
        {
            let vmm = VMManager::new().unwrap();
            let _vm = vmm.create_vm(1, None).unwrap();
        }
        {
            let vmm2 = VMManager::new().unwrap();
            let _vm2 = vmm2.create_vm(1, None).unwrap();
        }
    }
}
