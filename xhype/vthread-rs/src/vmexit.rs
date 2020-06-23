#[allow(unused_imports)]
use super::consts::msr::*;
#[allow(unused_imports)]
use super::hv::vmx::*;
#[allow(unused_imports)]
use super::x86::*;
use super::{Error, GuestThread, HandleResult, X86Reg, VCPU};
use log::{info, warn};

fn get_creg(num: u64) -> X86Reg {
    match num {
        0 => X86Reg::CR0,
        4 => X86Reg::CR4,
        _ => unreachable!(),
    }
}

pub fn handle_cr(vcpu: &VCPU, _gth: &GuestThread) -> Result<HandleResult, Error> {
    let qual = vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?;
    let creg = get_creg(qual & 0xf);
    let access_type = (qual << 4) & 0b11;
    let lmsw_type = (qual << 6) & 0b1;
    let reg = get_guest_reg((qual << 8) & 0xf);
    let source_data = (qual << 16) & 0xffff;
    let old_value = vcpu.read_reg(creg)?;
    info!(
        "{:?}={:x}, access={:x}, lmsw_type={:x}, reg={:?}, source={:x}",
        creg, old_value, access_type, lmsw_type, reg, source_data
    );
    match access_type {
        0 => {
            // move to cr
            let new_value = vcpu.read_reg(reg)?;
            vcpu.write_reg(creg, new_value)?;
            if creg == X86Reg::CR0 {
                let mut efer = vcpu.read_vmcs(VMCS_GUEST_IA32_EFER)?;
                let long_mode = new_value & X86_CR0_PE > 0
                    && new_value & X86_CR0_PG > 0
                    && efer & X86_EFER_LME > 0;
                if long_mode && efer & X86_EFER_LMA == 0 {
                    efer |= X86_EFER_LMA;
                    vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, efer)?;
                    let mut ctrl_entry = vcpu.read_vmcs(VMCS_CTRL_VMENTRY_CONTROLS)?;
                    ctrl_entry |= VMENTRY_GUEST_IA32E;
                    vcpu.write_vmcs(VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry)?;
                    info!("turn on LMA");
                }
                if !long_mode && efer & X86_EFER_LMA > 0 {
                    efer &= !X86_EFER_LMA;
                    vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, efer)?;
                    let mut ctrl_entry = vcpu.read_vmcs(VMCS_CTRL_VMENTRY_CONTROLS)?;
                    ctrl_entry &= !VMENTRY_GUEST_IA32E;
                    vcpu.write_vmcs(VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry)?;
                    info!("turn off LMA");
                }
            }
            info!("update {:?} to {:x}", creg, new_value);
        }
        _ => unimplemented!(),
    }

    Ok(HandleResult::Next)
}

//
// Handle MSR
//

struct MSRHander(
    pub u32,
    pub fn(u32, bool, u64, &VCPU, &GuestThread) -> Result<HandleResult, Error>,
);

#[inline]
fn write_msr_to_reg(msr_value: u64, vcpu: &VCPU) -> Result<HandleResult, Error> {
    let new_eax = msr_value & 0xffffffff;
    let new_edx = msr_value >> 32;
    vcpu.write_reg(X86Reg::RAX, new_eax)?;
    vcpu.write_reg(X86Reg::RDX, new_edx)?;
    info!("return msr value = {:x}", msr_value);
    Ok(HandleResult::Next)
}

fn emsr_unimpl(
    _msr: u32,
    _read: bool,
    _new_value: u64,
    _vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    unimplemented!()
}
/*
 * Set mandatory bits
 *  11:   branch trace disabled
 *  12:   PEBS unavailable
 * Clear unsupported features
 *  16:   SpeedStep enable
 *  18:   enable MONITOR FSM
 */
// FIX ME!
fn emsr_miscenable(
    _msr: u32,
    read: bool,
    new_value: u64,
    vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let misc_enable = 1 | ((1 << 12) | (1 << 11)) & !((1 << 18) | (1 << 16));
    if read {
        write_msr_to_reg(misc_enable, vcpu)
    } else {
        if new_value == misc_enable {
            Ok(HandleResult::Next)
        } else {
            Err(Error::Unhandled(
                VMX_REASON_WRMSR,
                "write a different value to misc_enable",
            ))
        }
    }
}

fn emsr_efer(
    _msr: u32,
    read: bool,
    new_value: u64,
    vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    if read {
        let value = vcpu.read_vmcs(VMCS_GUEST_IA32_EFER)?;
        write_msr_to_reg(value, vcpu)
    } else {
        vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, new_value)?;
        Ok(HandleResult::Next)
    }
}

fn emsr_rdonly(
    msr: u32,
    read: bool,
    new_value: u64,
    vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    if read {
        write_msr_to_reg(0, vcpu)
    } else {
        warn!("write {:x} to read-only msr {:x}", new_value, msr);
        Ok(HandleResult::Next)
    }
}

const MSR_HANDLERS: [MSRHander; 4] = [
    MSRHander(MSR_IA32_BIOS_SIGN_ID, emsr_rdonly),
    MSRHander(MSR_IA32_MISC_ENABLE, emsr_miscenable),
    MSRHander(MSR_LAPIC_ICR, emsr_unimpl),
    MSRHander(MSR_EFER, emsr_efer),
];

pub fn handle_msr_access(
    read: bool,
    vcpu: &VCPU,
    gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let ecx = vcpu.read_reg(X86Reg::RCX)? as u32;
    let new_value = if !read {
        let rdx = vcpu.read_reg(X86Reg::RDX)?;
        let rax = vcpu.read_reg(X86Reg::RAX)?;
        let v = (rdx << 32) | rax;
        info!("write msr = {:x}, new_value = {:x}", ecx, v);
        v
    } else {
        info!("read msr = {:x}", ecx);
        0
    };
    for handler in MSR_HANDLERS.iter() {
        if handler.0 == ecx {
            return handler.1(ecx, read, new_value, vcpu, gth);
        }
    }
    if read {
        Err(Error::Unhandled(VMX_REASON_RDMSR, "unkown msr"))
    } else {
        Err(Error::Unhandled(VMX_REASON_WRMSR, "unkown msr"))
    }
}
