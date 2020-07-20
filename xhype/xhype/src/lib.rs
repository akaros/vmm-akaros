/* SPDX-License-Identifier: GPL-2.0-only */

pub mod bios;
pub mod consts;
pub mod cpuid;
pub mod err;
pub mod hv;
pub mod linux;
pub mod mach;
pub mod serial;
pub mod utils;
pub mod vmexit;
pub mod vthread;

use err::Error;
use hv::ffi::{HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE};
use hv::vmx::*;
use hv::{MemSpace, X86Reg, DEFAULT_MEM_SPACE, VCPU};
#[allow(unused_imports)]
use log::*;
use mach::{vm_self_region, MachVMBlock};
use serial::Serial;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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

    pub fn create_vm(&self, cores: u32) -> Result<VirtualMachine, Error> {
        assert_eq!(cores, 1); //FIXME: currently only one core is supported
        VirtualMachine::new(&self, cores)
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

/// A VirtualMachine is the physical hardware seen by a guest, including physical
/// memory, number of cpu cores, etc.
pub struct VirtualMachine {
    pub(crate) mem_space: RwLock<MemSpace>,
    cores: u32,
    // the memory that is specifically allocated for the guest. For a vthread,
    // it contains its stack and a paging structure. For a kernel, it contains
    // its bios tables, APIC pages, high memory, etc.
    // the format is: guest virtual address -> host memory block
    pub(crate) mem_maps: RwLock<HashMap<usize, MachVMBlock>>,
    threads: Option<Vec<GuestThread>>,
    // serial ports
    pub(crate) com1: RwLock<Serial>,
}

impl VirtualMachine {
    // make it private to force user to create a vm by calling create_vm to make
    // sure that hv_vm_create() is called before hv_vm_space_create() is called
    fn new(_vmm: &VMManager, cores: u32) -> Result<Self, Error> {
        let mut vm = VirtualMachine {
            mem_space: RwLock::new(MemSpace::create()?),
            cores,
            mem_maps: RwLock::new(HashMap::new()),
            threads: None,
            com1: RwLock::new(Serial::default()),
        };
        vm.gpa2hva_map()?;
        Ok(vm)
    }

    pub(crate) fn map_guest_mem(&self, maps: HashMap<usize, MachVMBlock>) -> Result<(), Error> {
        let mut mem_space = self.mem_space.write().unwrap();
        for (gpa, mem_block) in maps.iter() {
            info!(
                "map gpa={:x} to hva={:x}, size={} pages",
                gpa,
                mem_block.start,
                mem_block.size / 4096
            );
            mem_space.map(
                mem_block.start,
                *gpa,
                mem_block.size,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
            )?;
        }
        *self.mem_maps.write().unwrap() = maps;
        Ok(())
    }

    // setup the identity mapping from guest physical address to host virtual
    // address.
    fn gpa2hva_map(&mut self) -> Result<(), Error> {
        let mut trial_addr = 1;
        let mut mem_space = self.mem_space.write().unwrap();
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
}

impl GuestThread {
    pub fn new(vm: &Arc<VirtualMachine>, id: u32) -> Self {
        GuestThread {
            vm: Arc::clone(vm),
            id: id,
            init_vmcs: HashMap::new(),
            init_regs: HashMap::new(),
            pat_msr: 0,
        }
    }

    pub fn start(mut self) -> std::thread::JoinHandle<Result<(), Error>> {
        std::thread::spawn(move || {
            let vcpu = VCPU::create()?;
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
            vcpu.run()?;
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
                VMX_REASON_IRQ => HandleResult::Resume,
                VMX_REASON_CPUID => handle_cpuid(&vcpu, self)?,
                VMX_REASON_HLT => HandleResult::Exit,
                VMX_REASON_MOV_CR => handle_cr(vcpu, self)?,
                VMX_REASON_RDMSR => handle_msr_access(vcpu, self, true)?,
                VMX_REASON_WRMSR => handle_msr_access(vcpu, self, false)?,
                VMX_REASON_IO => handle_io(vcpu, self)?,
                VMX_REASON_EPT_VIOLATION => {
                    let ept_gpa = vcpu.read_vmcs(VMCS_GUEST_PHYSICAL_ADDRESS)?;
                    if cfg!(debug_assertions) {
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
                    handle_ept_violation(vcpu, self, ept_gpa as usize)?
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
            let _vm = vmm.create_vm(1).unwrap();
        }
        {
            let vmm2 = VMManager::new().unwrap();
            let _vm2 = vmm2.create_vm(1).unwrap();
        }
    }
}
