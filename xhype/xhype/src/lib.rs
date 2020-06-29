#![cfg_attr(feature = "vthread_closure", feature(fn_traits))]
#[allow(dead_code)]
mod bios;
#[allow(non_upper_case_globals)]
pub mod consts;
mod cpuid;
mod decode;
pub mod err;
#[allow(dead_code)]
mod hv;
mod ioapic;
#[allow(dead_code)]
pub mod linux;
#[allow(non_camel_case_types)]
#[allow(dead_code)]
mod mach;
pub mod utils;

#[allow(dead_code)]
pub mod virtio;
mod vmexit;
pub mod vthread;
#[allow(dead_code)]
mod x86;
#[allow(unused_imports)]
use consts::msr::*;
use cpuid::do_cpuid;
use err::Error;
use hv::vmx::*;
use hv::X86Reg;
use hv::{MemSpace, DEFAULT_MEM_SPACE, HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE, VCPU};
use ioapic::IoApic;
#[allow(unused_imports)]
use log::*;
use mach::{vm_self_region, MachVMBlock};
use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use vmexit::*;
use x86::*;

////////////////////////////////////////////////////////////////////////////////
// VMManager
////////////////////////////////////////////////////////////////////////////////

// only one vmm is allowed to be created per process
pub struct VMManager {
    marker: PhantomData<()>, // add a PhantomData here to prevent user from constructing VMM by VMManager{}
    x86_host_xcr0: u64,
}

impl VMManager {
    pub fn new() -> Result<Self, Error> {
        hv::vm_create(0)?;
        let (eax, _, _, edx) = do_cpuid(0xd, 0x0);
        let proc_supported_features = (edx as u64) << 32 | (eax as u64);
        Ok(VMManager {
            marker: PhantomData,
            x86_host_xcr0: proc_supported_features & X86_MAX_XCR0,
        })
    }

    pub fn create_vm(&self, cores: u32) -> Result<VirtualMachine, Error> {
        assert_eq!(cores, 1); //FIXME: currently only one core is supported
        VirtualMachine::new(cores, &self)
    }
}

// let rust call hv_vm_destroy automatically
impl Drop for VMManager {
    fn drop(&mut self) {
        hv::vm_destroy().unwrap();
    }
}

////////////////////////////////////////////////////////////////////////////////
// VirtualMachine
////////////////////////////////////////////////////////////////////////////////

/// A VirtualMachine is the physical hardware seen by a guest, including physical
/// memory, number of cpu cores, etc.
pub struct VirtualMachine {
    mem_space: MemSpace,
    cores: u32,
    // fixme: add lock to pci ports?
    pub(crate) cf8: u32,
    pub(crate) host_bridge_data: [u32; 16],
    pub(crate) ioapic: Arc<RwLock<IoApic>>,
    /// the memory that is specifically allocated for the guest. For a vthread,
    /// it contains its stack and a paging structure. For a kernel, it contains
    /// its bios tables, APIC pages, high memory, etc.
    /// guest virtual address -> host VM block
    pub(crate) guest_mmap: HashMap<usize, MachVMBlock>,
    pub vmcall_hander: fn(&VCPU, &GuestThread) -> Result<HandleResult, Error>,
    x86_host_xcr0: u64,
}

impl VirtualMachine {
    // make it private to force user to create a vm by calling create_vm to make
    // sure that hv_vm_create() is called before hv_vm_space_create() is called
    fn new(cores: u32, vmm: &VMManager) -> Result<Self, Error> {
        let mut host_bridge_data = [0; 16];
        let data = [0x71908086, 0x02000006, 0x06000001]; //0:00.0 Host bridge: Intel Corporation 440BX/ZX/DX - 82443BX/ZX/DX Host bridge (rev 01)
        for (i, n) in data.iter().enumerate() {
            host_bridge_data[i] = *n;
        }
        let mut vm = VirtualMachine {
            mem_space: MemSpace::create()?,
            cores,
            cf8: 0,
            host_bridge_data,
            ioapic: Arc::new(RwLock::new(IoApic::new())),
            guest_mmap: HashMap::new(),
            vmcall_hander: default_vmcall_handler,
            x86_host_xcr0: vmm.x86_host_xcr0,
        };
        vm.gpa2hva_map()?;
        Ok(vm)
    }

    fn map_guest_mem(&mut self, maps: HashMap<usize, MachVMBlock>) -> Result<(), Error> {
        self.guest_mmap = maps;
        for (gpa, mem_block) in self.guest_mmap.iter() {
            info!(
                "map gpa={:x} to hva={:x}, size={}page",
                gpa,
                mem_block.start,
                mem_block.size / 4096
            );
            self.mem_space.map(
                mem_block.start,
                *gpa,
                mem_block.size,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
            )?;
        }
        Ok(())
    }

    fn gpa2hva_map(&mut self) -> Result<(), Error> {
        let mut trial_addr = 1;
        loop {
            match vm_self_region(trial_addr) {
                Ok((start, size, info)) => {
                    if info.protection > 0 {
                        self.mem_space
                            .map(start, start, size, info.protection as u64)?;
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
}

////////////////////////////////////////////////////////////////////////////////
// GuestThread
////////////////////////////////////////////////////////////////////////////////

pub struct GuestThread {
    pub vm: Arc<RwLock<VirtualMachine>>,
    pub id: u32,
    pub init_vmcs: HashMap<u32, u64>,
    pub init_regs: HashMap<X86Reg, u64>,
    vapic_addr: usize,
    posted_irq_desc: usize,
    pub(crate) msr_pat: Cell<u64>,
}

impl GuestThread {
    pub fn new(vm: &Arc<RwLock<VirtualMachine>>, id: u32) -> Self {
        GuestThread {
            vm: Arc::clone(vm),
            id: id,
            init_vmcs: HashMap::new(),
            init_regs: HashMap::new(),
            vapic_addr: 0,
            posted_irq_desc: 0,
            msr_pat: Cell::new(0x7040600070406),
        }
    }

    pub fn start(self) -> std::thread::JoinHandle<Result<(), Error>> {
        std::thread::spawn(move || {
            let vcpu = VCPU::create()?;
            self.run_on(&vcpu)
        })
    }
    pub(crate) fn run_on(&self, vcpu: &VCPU) -> Result<(), Error> {
        {
            let mem_space = &(self.vm.read().unwrap()).mem_space;
            vcpu.set_space(mem_space)?;
            trace!("set vcpu {} space to {}", vcpu.id(), mem_space.id);
        }
        let result = self.run_on_inner(vcpu);
        vcpu.set_space(&DEFAULT_MEM_SPACE)?;
        trace!("set vcpu back {} space to 0", vcpu.id());
        result
    }
    fn run_on_inner(&self, vcpu: &VCPU) -> Result<(), Error> {
        vcpu.set_vapic_address(self.vapic_addr)?;
        vcpu.enable_msrs()?;
        vcpu.long_mode()?;
        for (field, value) in self.init_vmcs.iter() {
            vcpu.write_vmcs(*field, *value)?;
        }
        for (reg, value) in self.init_regs.iter() {
            vcpu.write_reg(*reg, *value)?;
        }

        // vcpu.dump().unwrap();
        let mut result: HandleResult;
        let mut last_physical_addr = 0;
        let mut ept_count = 0;
        loop {
            vcpu.run()?;
            let reason = vcpu.read_vmcs(VMCS_RO_EXIT_REASON)?;
            trace!("vm exit reason = {}", reason);
            let instr_len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
            result = match reason {
                VMX_REASON_EXC_NMI => {
                    let info = vcpu.read_vmcs(VMCS_RO_VMEXIT_IRQ_INFO)?;
                    let code = vcpu.read_vmcs(VMCS_RO_VMEXIT_IRQ_ERROR)?;
                    let valid = (info >> 31) & 1 == 1;
                    let nmi = (info >> 12) & 1 == 1;
                    let e_type = (info >> 8) & 0b111;
                    let vector = info & 0xf;
                    info!(
                        "VMX_REASON_EXC_NMI, valid = {}, nmi = {}, type = {}, vector = {}, code = {:b}",
                        valid, nmi, e_type, vector, code
                    );
                    return Err(Error::Unhandled(reason, "unhandled exception"));
                }
                VMX_REASON_IRQ => HandleResult::Resume,
                VMX_REASON_CPUID => handle_cpuid(&vcpu, self),
                VMX_REASON_HLT => HandleResult::Exit,
                VMX_REASON_VMCALL => handle_vmcall(&vcpu, self)?,
                VMX_REASON_MOV_CR => handle_cr(&vcpu, self)?,
                VMX_REASON_IO => handle_io(&vcpu, self)?,
                VMX_REASON_RDMSR => handle_msr_access(true, &vcpu, self)?,
                VMX_REASON_WRMSR => handle_msr_access(false, &vcpu, self)?,
                VMX_REASON_EPT_VIOLATION => {
                    let physical_addr = vcpu.read_vmcs(VMCS_GUEST_PHYSICAL_ADDRESS)?;
                    if physical_addr == last_physical_addr {
                        ept_count += 1;
                    } else {
                        ept_count = 0;
                        last_physical_addr = physical_addr;
                    }
                    if ept_count > 10 {
                        error!(
                            "EPT violation at {:x} for {} times",
                            last_physical_addr, ept_count
                        );
                        return Err(Error::Unhandled(
                            reason,
                            "too many EPT faults at the same address",
                        ));
                    } else {
                        handle_ept_violation(physical_addr as usize, vcpu, self)?
                    }
                }
                VMX_REASON_XSETBV => handle_xsetbv(&vcpu, self)?,
                _ => {
                    trace!("Unhandled reason = {}", reason);
                    if reason < VMX_REASON_MAX {
                        return Err(Error::Unhandled(reason, "unable to handle"));
                    } else {
                        return Err(Error::Unhandled(reason, "unknown reason"));
                    }
                }
            };
            match result {
                HandleResult::Exit => break,
                HandleResult::Next => {
                    let rip = vcpu.read_reg(X86Reg::RIP)?;
                    vcpu.write_reg(X86Reg::RIP, rip + instr_len)?;
                }
                HandleResult::Resume => (),
            };
        }
        Ok(())
    }
}

extern "C" {
    pub fn hlt();
    pub fn raw_vmcall(num: u64, args: *const u8);
}

/// num is the function number, args is a pointer to arguments
/// currently the following functions are supported:
/// num = 1, args = pointer to a c-style string: print the string
pub fn vmcall(num: u64, args: *const u8) {
    unsafe {
        raw_vmcall(num, args);
    }
}
