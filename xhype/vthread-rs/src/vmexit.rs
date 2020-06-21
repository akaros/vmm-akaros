#[allow(unused_imports)]
use super::hv::vmx::*;
#[allow(unused_imports)]
use super::x86::*;
use super::{Error, GuestThread, HandleResult, X86Reg, VCPU};
use log::info;
fn get_creg(num: u64) -> X86Reg {
    match num {
        0 => X86Reg::CR0,
        4 => X86Reg::CR4,
        _ => unreachable!(),
    }
}

pub fn handle_cr(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
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
