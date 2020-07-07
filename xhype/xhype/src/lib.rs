/* SPDX-License-Identifier: GPL-2.0-only */

pub mod bios;
pub mod consts;
pub mod cpuid;
pub mod err;
pub mod hv;
pub mod linux;
pub mod mach;

use err::Error;
use hv::ffi::{HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE};
use hv::{MemSpace, X86Reg};
#[allow(unused_imports)]
use log::*;
use mach::{vm_self_region, MachVMBlock};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
    pub(crate) mem_space: MemSpace,
    cores: u32,
    // the memory that is specifically allocated for the guest. For a vthread,
    // it contains its stack and a paging structure. For a kernel, it contains
    // its bios tables, APIC pages, high memory, etc.
    // the format is: guest virtual address -> host memory block
    pub(crate) mem_maps: HashMap<usize, MachVMBlock>,
    threads: Option<Vec<GuestThread>>,
}

impl VirtualMachine {
    // make it private to force user to create a vm by calling create_vm to make
    // sure that hv_vm_create() is called before hv_vm_space_create() is called
    fn new(_vmm: &VMManager, cores: u32) -> Result<Self, Error> {
        let mut vm = VirtualMachine {
            mem_space: MemSpace::create()?,
            cores,
            mem_maps: HashMap::new(),
            threads: None,
        };
        vm.gpa2hva_map()?;
        Ok(vm)
    }

    pub(crate) fn map_guest_mem(&mut self, maps: HashMap<usize, MachVMBlock>) -> Result<(), Error> {
        self.mem_maps = maps;
        for (gpa, mem_block) in self.mem_maps.iter() {
            info!(
                "map gpa={:x} to hva={:x}, size={} pages",
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

    // setup the identity mapping from guest physical address to host virtual
    // address.
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
}

impl GuestThread {
    pub fn new(vm: &Arc<RwLock<VirtualMachine>>, id: u32) -> Self {
        GuestThread {
            vm: Arc::clone(vm),
            id: id,
            init_vmcs: HashMap::new(),
            init_regs: HashMap::new(),
        }
    }
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
