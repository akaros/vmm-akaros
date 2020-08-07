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
pub mod multiboot;
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
use std::fs::File;
use std::io::Read;
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

type AddressConverter = Arc<dyn Fn(u64) -> usize + Send + Sync>;

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
    pub low_mem: Option<RwLock<MachVMBlock>>, // memory below 4GiB
    pub(crate) high_mem: RwLock<Vec<MachVMBlock>>,
    pub(crate) virtio_mmio_devices: Vec<Mutex<VirtioMmioDev>>,
    // serial ports
    pub(crate) com1: Mutex<Serial>,
    pub(crate) ioapic: Arc<RwLock<IoApic>>,
    pub vector_senders: Arc<Mutex<Option<Vec<Sender<u8>>>>>,
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
            (Some(RwLock::new(low_mem_block)), Arc::new(converter))
        } else {
            virtio_base = GiB; // just use the 1 GiB address as virtio base
            let converter = |gpa: u64| gpa as usize;
            (None, Arc::new(converter))
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
        if result.is_err() {
            vcpu.dump().unwrap();
        }
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
            /*
            This is the way we implement APIC timer. We setup a deadline and wait
            for a `VMX_REASON_VMX_TIMER_EXPIRED`. See more details in the comments
            in apic.rs.

            Notice that in the Intel's manual, VMX preemption timer loads a value
            from VMX-preemption timer-value field, which represents a timer interval,
            and then count down until 0. However the function vcpu.run_until accepts
            a deadline and the Hypervisor framework calculates the correct time
            interval for us.
            */
            if let Some(deadline) = self.apic.sched_timer {
                vcpu.run_until(deadline)?;
            } else {
                /*
                According to Apple's doc(https://developer.apple.com/documentation/hypervisor/1441231-hv_vcpu_run),
                it is recommended to use `hv_vcpu_run_until(HV_DEADLINE_FOREVER)`
                instead of `hv_vcpu_run`. While in the doc `HV_DEADLINE_FOREVER`
                is only available after macOS 11, but as I tested, it still
                works on macOS 10.15.

                Why we do not use `vcpu.run()`, or hv_vcpu_run() ?
                As I observed, hv_vcpu_run will cause the two following vm spurious exits:
                1. VMX_REASON_EPT_VIOLATION
                This is related to how Apple manages EPT tables. `hv_vm_map` only
                remembers which block of host virtual memory should
                mapped to which block of guest physical memory, but it does NOT
                really setup the EPT table. So When a EPT violation happens, in the macOS kernel
                space, the Hypervisor framework will first check if the fault guest address
                is mapped to some host address, if there is not, it returns this
                vm exit back to user space. Otherwise, it sets up the EPT entry.

                So the important thing here is: what would the Hypervisor framework
                do after it sets up the EPT entry:
                    * If we use `vcpu.run`, then the function returns and we got a `VMX_REASON_EPT_VIOLATION`.
                    * If we use `vcpu.run_until`, then the function goes back
                    to VMX non-root mode and continue executing VM codes. We will
                    not see a `VMX_REASON_EPT_VIOLATION`.

                2. VMX_REASON_IRQ
                See more details in the comment for `struct Apic` in `apic.rs`.
                    * If we use `vcpu.run`, then the timer interrupts from the
                    REAL apic timer will cause vm exit and eventually deliver
                    the interrupt to user space.
                    * If we use `vcpu.run_until`, then those interrupts will be
                    handled by the Hypervisor framework itself and as a user-space
                    program, xhype will not see those `VMX_REASON_IRQ`.

                */
                vcpu.run_until(u64::MAX)?;
            }
            let reason = vcpu.read_vmcs(VMCS_RO_EXIT_REASON)?;
            let rip = vcpu.read_reg(X86Reg::RIP)?;
            let instr_len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
            trace!(
                "vm exit reason = {}, cs = {:x}, cs base = {:x}, rip = {:x}, len = {}",
                reason,
                vcpu.read_reg(X86Reg::CS)?,
                vcpu.read_vmcs(VMCS_GUEST_CS_BASE)?,
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
                    debug_assert!(self.apic.sched_timer.is_some());
                    self.apic.fire_timer_interrupt(vcpu);
                    HandleResult::Resume
                }
                VMX_REASON_MTF => {
                    if let Ok(mtf_fifo) = std::env::var("DEBUG_FIFO") {
                        println!(
                            "cs_base = {:x}, rip = {:x}",
                            vcpu.read_vmcs(VMCS_GUEST_CS_BASE)?,
                            vcpu.read_reg(X86Reg::RIP)?
                        );
                        println!(
                            "write any ONE byte to {} to continue, e.g. echo 'a' > {}",
                            &mtf_fifo, &mtf_fifo
                        );
                        let mut f = File::open(&mtf_fifo).unwrap();
                        let mut buf = [0u8];
                        f.read_exact(&mut buf).unwrap();
                    }
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
                                "EPT violation at physical address {:x} for {} times, cs base = {:x}; linear addr = {:x} ",
                                last_ept_gpa, ept_count, vcpu.read_vmcs(VMCS_GUEST_CS_BASE)?, vcpu.read_vmcs(VMCS_RO_GUEST_LIN_ADDR)?,
                            );
                            error!("{}", &err_msg);
                            let qual = vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?;
                            debug!("{}", vmexit::ept_qual_description(qual));
                            let linear_addr = vcpu.read_vmcs(VMCS_RO_GUEST_LIN_ADDR)?;
                            // the following instruction may cause segment fault.
                            let simulate_physical_addr =
                                unsafe { vmexit::emulate_paging(vcpu, self, linear_addr) };
                            debug!("emulated paging result: {:x?}", simulate_physical_addr);
                            return Err((reason, err_msg))?;
                        }
                    }
                    ret
                }
                _ => {
                    let err_msg =
                        format!("handler for vm exit code 0x{:x} is not implemented", reason);
                    error!("{}", err_msg);
                    if reason & (1 << 31) > 0 {
                        vcpu.dump().unwrap();
                    }
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
