#[allow(unused_imports)]
use super::hv::vmx::*;
#[allow(unused_imports)]
use super::x86::*;
use super::{Error, GuestThread, HandleResult, X86Reg, VCPU};
use log::{info, warn};

pub fn handle_wrmsr(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let ecx = vcpu.read_reg(X86Reg::RCX)? as u32;
    let rdx = vcpu.read_reg(X86Reg::RDX)?;
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let new_msr = (rdx << 32) | rax;
    info!("ecx = {:x}, new_msr = {:x}", ecx, new_msr);
    match ecx {
        MSR_EFER => vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, new_msr)?,
        _ => return Err(Error::Unhandled(VMX_REASON_WRMSR, "handle_wrmsr")),
    }
    Ok(HandleResult::Next)
}

pub fn handle_rdmsr(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let ecx = vcpu.read_reg(X86Reg::RCX)? as u32;
    let new_value = match ecx {
        MSR_EFER => vcpu.read_vmcs(VMCS_GUEST_IA32_EFER)?,
        _ => return Err(Error::Unhandled(VMX_REASON_RDMSR, "handle_rdmsr")),
    };
    let new_eax = new_value & 0xffffffff;
    let new_edx = new_value >> 32;
    vcpu.write_reg(X86Reg::RAX, new_eax)?;
    vcpu.write_reg(X86Reg::RDX, new_edx)?;
    Ok(HandleResult::Next)
}
