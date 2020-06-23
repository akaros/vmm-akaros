#![cfg_attr(feature = "vthread_closure", feature(fn_traits))]
#[allow(non_upper_case_globals)]
pub mod consts;
mod cpuid;
pub mod err;
#[allow(dead_code)]
mod hv;
#[allow(dead_code)]
pub mod loader;
#[allow(non_camel_case_types)]
mod mach;
mod paging;
mod vmexit;
pub mod vthread;
#[allow(dead_code)]
mod x86;
#[allow(unused_imports)]
use consts::msr::*;
use err::Error;
use hv::vmx::*;
use hv::X86Reg;
use hv::{
    cap2ctrl, vmx_read_capability, MemSpace, VMXCap, HV_MEMORY_EXEC, HV_MEMORY_READ,
    HV_MEMORY_WRITE, VCPU,
};
use mach::{vm_self_region, MachVMBlock};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use vmexit::*;
use x86::*;

// only one vmm is allowed to be created per process
pub struct VMManager {
    marker: PhantomData<()>, // add a PhantomData here to prevent user from constructing VMM by VMManager{}
}

impl VMManager {
    pub fn new() -> Result<Self, Error> {
        hv::vm_create(0)?;
        Ok(VMManager {
            marker: PhantomData,
        })
    }

    pub fn create_vm(&self, cores: u32) -> Result<VirtualMachine, Error> {
        assert_eq!(cores, 1); //FIXME: currently only one core is supported
        VirtualMachine::new(cores)
    }
    pub fn create_vcpu(&self) -> Result<VCPU, Error> {
        VCPU::create()
    }
}

// let rust call hv_vm_destroy automatically
impl Drop for VMManager {
    fn drop(&mut self) {
        hv::vm_destroy().unwrap();
    }
}

#[derive(Debug)]
pub struct VirtualMachine {
    mem_space: MemSpace,
    cores: u32,
}

impl VirtualMachine {
    // make it private to force user to create a vm by calling create_vm to make
    // sure that hv_vm_create() is called before hv_vm_space_create() is called
    fn new(cores: u32) -> Result<Self, Error> {
        let vm = VirtualMachine {
            mem_space: MemSpace::create()?,
            cores,
        };
        vm.gpa2hva_map()?;
        Ok(vm)
    }

    fn gpa2hva_map(&self) -> Result<(), Error> {
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

#[derive(Debug)]
pub struct GuestThread {
    pub vm: Arc<RwLock<VirtualMachine>>,
    pub init_vmcs: HashMap<u32, u64>,
    pub init_regs: HashMap<X86Reg, u64>,

    pub mem_maps: HashMap<usize, MachVMBlock>, // gpa -> host VM block
}

impl VCPU {
    fn longmode(&self) -> Result<(), Error> {
        self.enable_native_msr(MSR_LSTAR, true)?;
        self.enable_native_msr(MSR_CSTAR, true)?;
        self.enable_native_msr(MSR_STAR, true)?;
        self.enable_native_msr(MSR_SYSCALL_MASK, true)?;
        self.enable_native_msr(MSR_KERNEL_GS_BASE, true)?;
        self.enable_native_msr(MSR_GS_BASE, true)?;
        self.enable_native_msr(MSR_FS_BASE, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_CS, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_ESP, true)?;
        self.enable_native_msr(MSR_IA32_SYSENTER_EIP, true)?;
        self.enable_native_msr(MSR_IA32_TSC, true)?;
        self.enable_native_msr(MSR_TSC_AUX, true)?;

        self.write_vmcs(VMCS_GUEST_CS, 0x10)?;
        self.write_vmcs(VMCS_GUEST_CS_AR, 0xa09b)?;
        self.write_vmcs(VMCS_GUEST_CS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_CS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_DS, 0x18)?;
        self.write_vmcs(VMCS_GUEST_DS_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_DS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_DS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_ES, 0x18)?;
        self.write_vmcs(VMCS_GUEST_ES_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_ES_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_ES_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_FS, 0)?;
        self.write_vmcs(VMCS_GUEST_FS_AR, 0x93)?;
        self.write_vmcs(VMCS_GUEST_FS_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_FS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_GS, 0)?;
        self.write_vmcs(VMCS_GUEST_GS_AR, 0x93)?;
        self.write_vmcs(VMCS_GUEST_GS_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_GS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_SS, 0x18)?;
        self.write_vmcs(VMCS_GUEST_SS_AR, 0xc093)?;
        self.write_vmcs(VMCS_GUEST_SS_LIMIT, 0xffffffff)?;
        self.write_vmcs(VMCS_GUEST_SS_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_LDTR, 0)?;
        self.write_vmcs(VMCS_GUEST_LDTR_AR, 0x82)?;
        self.write_vmcs(VMCS_GUEST_LDTR_LIMIT, 0xffff)?;
        self.write_vmcs(VMCS_GUEST_LDTR_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_GDTR_BASE, 0x17)?;
        self.write_vmcs(VMCS_GUEST_GDTR_LIMIT, 0xfe0)?;

        self.write_vmcs(VMCS_GUEST_TR, 0)?;
        self.write_vmcs(VMCS_GUEST_TR_AR, 0x8b)?;
        self.write_vmcs(VMCS_GUEST_TR_LIMIT, 0)?;
        self.write_vmcs(VMCS_GUEST_TR_BASE, 0)?;

        self.write_vmcs(VMCS_GUEST_IDTR_LIMIT, 0)?;
        self.write_vmcs(VMCS_GUEST_IDTR_BASE, 0)?;

        let cap_pin = vmx_read_capability(VMXCap::PIN)?;
        let cap_cpu = vmx_read_capability(VMXCap::CPU)?;
        let cap_cpu2 = vmx_read_capability(VMXCap::CPU2)?;
        let cap_entry = vmx_read_capability(VMXCap::ENTRY)?;

        self.write_vmcs(VMCS_CTRL_PIN_BASED, cap2ctrl(cap_pin, 0))?;
        self.write_vmcs(
            VMCS_CTRL_CPU_BASED,
            cap2ctrl(
                cap_cpu,
                CPU_BASED_HLT | CPU_BASED_CR8_LOAD | CPU_BASED_CR8_STORE,
            ),
        )?;
        self.write_vmcs(VMCS_CTRL_CPU_BASED2, cap2ctrl(cap_cpu2, CPU_BASED2_RDTSCP))?;
        self.write_vmcs(
            VMCS_CTRL_VMENTRY_CONTROLS,
            cap2ctrl(cap_entry, VMENTRY_GUEST_IA32E),
        )?;

        self.write_vmcs(VMCS_CTRL_EXC_BITMAP, 0xffffffff)?;

        let cr0 = X86_CR0_NE | X86_CR0_ET | X86_CR0_PE | X86_CR0_PG;
        self.write_vmcs(VMCS_GUEST_CR0, cr0)?;
        self.write_vmcs(VMCS_CTRL_CR0_MASK, X86_CR0_PE | X86_CR0_PG)?;
        self.write_vmcs(VMCS_CTRL_CR0_SHADOW, X86_CR0_PE | X86_CR0_PG)?;

        let cr4 = X86_CR4_VMXE | X86_CR4_OSFXSR | X86_CR4_OSXSAVE | X86_CR4_PAE;
        self.write_vmcs(VMCS_GUEST_CR4, cr4)?;
        self.write_vmcs(VMCS_CTRL_CR4_MASK, X86_CR4_VMXE)?;
        self.write_vmcs(VMCS_CTRL_CR4_SHADOW, 0)?;

        let efer = X86_EFER_LMA | X86_EFER_LME;
        self.write_vmcs(VMCS_GUEST_IA32_EFER, efer)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum HandleResult {
    Exit,
    Resume,
    Next,
}

impl GuestThread {
    pub fn run_on(&self, vcpu: &VCPU) -> Result<(), Error> {
        {
            vcpu.set_space(&(self.vm.read().unwrap()).mem_space)?;
        }
        {
            let vm = self.vm.write().unwrap();
            for (gpa, mem_block) in self.mem_maps.iter() {
                vm.mem_space.map(
                    mem_block.start,
                    *gpa,
                    mem_block.size,
                    HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
                )?;
            }
        }
        vcpu.longmode()?;
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
            let instr_len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
            result = match reason {
                VMX_REASON_EXC_NMI => return Err(Error::Unhandled(reason, "unhandled exception")),
                VMX_REASON_IRQ => HandleResult::Resume,
                VMX_REASON_CPUID => cpuid::handle_cpuid(&vcpu, self),
                VMX_REASON_HLT => HandleResult::Exit,
                VMX_REASON_MOV_CR => handle_cr(&vcpu, self)?,
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
                        return Err(Error::Unhandled(reason, "too many EPT at the same place"));
                    } else {
                        HandleResult::Resume
                    }
                }
                _ => {
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
}

#[cfg(test)]
mod tests {
    use super::vthread::VThread;
    use super::{VMManager, VCPU};

    static mut NUM_A: i32 = 1;
    extern "C" fn add_a() {
        unsafe {
            NUM_A += 3;
        }
    }

    use std::sync::{Arc, RwLock};
    #[test]
    fn vthread_test() {
        let vmm = VMManager::new().unwrap();
        let vm = Arc::new(RwLock::new(vmm.create_vm(1).unwrap()));
        let vth = VThread::create(&vm, add_a).unwrap();
        let vcpu = VCPU::create().unwrap();
        vth.gth.run_on(&vcpu).unwrap();
        unsafe {
            assert_eq!(NUM_A, 4);
        }
    }
}
