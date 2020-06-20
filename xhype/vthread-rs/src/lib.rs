#[allow(dead_code)]
mod hv;
#[allow(non_camel_case_types)]
mod mach;
mod paging;
pub mod vthread;
#[allow(dead_code)]
mod x86;
use hv::vmx::*;
use hv::X86Reg;
use hv::{
    cap2ctrl, vmx_read_capability, MemSpace, VMXCap, HV_MEMORY_EXEC, HV_MEMORY_READ,
    HV_MEMORY_WRITE, VCPU,
};
use mach::{vm_self_region, MachVMBlock};
use std::collections::HashMap;
use x86::*;

pub fn vmm_init() -> Result<(), u32> {
    hv::vm_create(0)
}

#[derive(Debug)]
pub struct VirtualMachine {
    mem_space: MemSpace,
}

impl VirtualMachine {
    pub fn new() -> Result<Self, u32> {
        let vm = VirtualMachine {
            mem_space: MemSpace::create()?,
        };
        vm.gpa2hva_map()?;
        Ok(vm)
    }

    fn gpa2hva_map(&self) -> Result<(), u32> {
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
pub struct GuestThread<'a> {
    pub vm: &'a VirtualMachine,
    pub init_vmcs: HashMap<u32, u64>,
    pub init_regs: HashMap<X86Reg, u64>,

    pub mem_maps: HashMap<usize, MachVMBlock>, // gpa -> host VM block
}

impl VCPU {
    fn longmode(&self) -> Result<(), u32> {
        self.enable_native_msr(MSR_LSTAR, true)?;
        self.enable_native_msr(MSR_CSTAR, true)?;
        self.enable_native_msr(MSR_STAR, true)?;
        self.enable_native_msr(MSR_SF_MASK, true)?;
        self.enable_native_msr(MSR_KGSBASE, true)?;
        self.enable_native_msr(MSR_GSBASE, true)?;
        self.enable_native_msr(MSR_FSBASE, true)?;
        self.enable_native_msr(MSR_SYSENTER_CS_MSR, true)?;
        self.enable_native_msr(MSR_SYSENTER_ESP_MSR, true)?;
        self.enable_native_msr(MSR_SYSENTER_EIP_MSR, true)?;
        self.enable_native_msr(MSR_TSC, true)?;
        self.enable_native_msr(MSR_IA32_TSC_AUX, true)?;

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
        self.write_vmcs(VMCS_CTRL_CR0_MASK, 0xe0000031)?;
        self.write_vmcs(VMCS_CTRL_CR0_SHADOW, 0)?;

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
    Abort(u64),
    Exit,
    Resume,
    Next,
}

impl<'a> GuestThread<'a> {
    pub fn run_on(&self, vcpu: &VCPU) -> Result<HandleResult, u32> {
        vcpu.set_space(&self.vm.mem_space)?;
        vcpu.longmode()?;
        for (field, value) in self.init_vmcs.iter() {
            vcpu.write_vmcs(*field, *value)?;
        }
        for (reg, value) in self.init_regs.iter() {
            vcpu.write_reg(*reg, *value)?;
        }
        for (gpa, mem_block) in self.mem_maps.iter() {
            self.vm.mem_space.map(
                mem_block.start,
                *gpa,
                mem_block.size,
                HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC,
            )?;
        }

        vcpu.dump().unwrap();
        let mut result: HandleResult;
        let mut last_physical_addr = 0;
        let mut ept_count = 0;
        loop {
            vcpu.run()?;
            let reason = vcpu.read_vmcs(VMCS_RO_EXIT_REASON)?;
            let instr_len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
            result = match reason {
                VMX_REASON_EXC_NMI => HandleResult::Abort(reason),
                VMX_REASON_IRQ => HandleResult::Resume,
                VMX_REASON_HLT => HandleResult::Exit,
                VMX_REASON_EPT_VIOLATION => {
                    let physical_addr = vcpu.read_vmcs(VMCS_GUEST_PHYSICAL_ADDRESS)?;
                    if physical_addr == last_physical_addr {
                        ept_count += 1;
                    } else {
                        ept_count = 0;
                        last_physical_addr = physical_addr;
                    }
                    if ept_count > 10 {
                        HandleResult::Abort(reason)
                    } else {
                        HandleResult::Resume
                    }
                }
                _ => {
                    if reason < VMX_REASON_MAX {
                        dbg!(reason);
                        HandleResult::Abort(reason)
                    } else {
                        return Err(reason as u32);
                    }
                }
            };
            match result {
                HandleResult::Abort(_) | HandleResult::Exit => break,
                HandleResult::Next => {
                    let rip = vcpu.read_reg(X86Reg::RIP)?;
                    vcpu.write_reg(X86Reg::RIP, rip + instr_len)?;
                }
                HandleResult::Resume => (),
            };
        }
        Ok(result)
    }
}

extern "C" {
    #[link(name = "libhlt")]
    pub fn hlt();
}

#[cfg(test)]
mod tests {
    use super::vthread::VThread;
    use super::{vmm_init, HandleResult, VirtualMachine, VCPU};

    static mut NUM_A: i32 = 1;
    extern "C" fn add_a() {
        unsafe {
            NUM_A += 3;
        }
    }

    #[test]
    fn vthread_test() {
        vmm_init().unwrap();
        let vm = VirtualMachine::new().unwrap();
        let vth = VThread::create(&vm, add_a).unwrap();
        let vcpu = VCPU::create().unwrap();
        assert_eq!(vth.gth.run_on(&vcpu), Ok(HandleResult::Exit));
        unsafe {
            assert_eq!(NUM_A, 4);
        }
    }
}
