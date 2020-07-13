/* SPDX-License-Identifier: GPL-2.0-only */

use crate::consts::msr::*;
use crate::consts::x86::*;
use crate::err::Error;
use crate::hv::vmx::*;
use crate::hv::X86Reg;
use crate::{GuestThread, VCPU};
#[allow(unused_imports)]
use log::*;

#[derive(Debug, Eq, PartialEq)]
pub enum HandleResult {
    Exit,
    Resume,
    Next,
}

// Table 27-3
#[inline]
pub fn vmx_guest_reg(num: u64) -> X86Reg {
    match num {
        0 => X86Reg::RAX,
        1 => X86Reg::RCX,
        2 => X86Reg::RDX,
        3 => X86Reg::RBX,
        4 => X86Reg::RSP,
        5 => X86Reg::RBP,
        6 => X86Reg::RSI,
        7 => X86Reg::RDI,
        8 => X86Reg::R8,
        9 => X86Reg::R9,
        10 => X86Reg::R10,
        11 => X86Reg::R11,
        12 => X86Reg::R12,
        13 => X86Reg::R13,
        14 => X86Reg::R14,
        15 => X86Reg::R15,
        _ => panic!("bad register in exit qualification"),
    }
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_MOV_CR
////////////////////////////////////////////////////////////////////////////////

// Intel SDM Table 27-3.
pub fn handle_cr(vcpu: &VCPU, _gth: &GuestThread) -> Result<HandleResult, Error> {
    let qual = vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?;
    let ctrl_reg = match qual & 0xf {
        0 => X86Reg::CR0,
        3 => X86Reg::CR3,
        4 => X86Reg::CR4,
        8 => {
            return Err(Error::Unhandled(
                VMX_REASON_MOV_CR,
                format!("access cr8: unimplemented"),
            ))
        }
        _ => unreachable!(), // Table C-1. Basic Exit Reasons (Contd.)
    };
    let access_type = (qual >> 4) & 0b11;
    let _lmsw_type = (qual >> 6) & 0b1;
    let reg = vmx_guest_reg((qual >> 8) & 0xf);
    let _source_data = (qual >> 16) & 0xffff;
    match access_type {
        0 => {
            // move to cr
            let mut new_value = vcpu.read_reg(reg)?;
            match ctrl_reg {
                X86Reg::CR0 => {
                    new_value |= X86_CR0_NE;
                    vcpu.write_vmcs(VMCS_CTRL_CR0_SHADOW, new_value)?;
                }
                _ => {
                    return Err(Error::Unhandled(
                        VMX_REASON_MOV_CR,
                        format!("mov to {:?}: unimplemented", ctrl_reg),
                    ));
                }
            }
            vcpu.write_reg(ctrl_reg, new_value)?;

            if ctrl_reg == X86Reg::CR0 || ctrl_reg == X86Reg::CR4 {
                let cr0 = vcpu.read_reg(X86Reg::CR0)?;
                let cr4 = vcpu.read_reg(X86Reg::CR4)?;
                let mut efer = vcpu.read_vmcs(VMCS_GUEST_IA32_EFER)?;
                let long_mode = cr0 & X86_CR0_PE != 0
                    && cr0 & X86_CR0_PG != 0
                    && cr4 & X86_CR4_PAE != 0
                    && efer & EFER_LME != 0;
                if long_mode && efer & EFER_LMA == 0 {
                    efer |= EFER_LMA;
                    vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, efer)?;
                    let mut ctrl_entry = vcpu.read_vmcs(VMCS_CTRL_VMENTRY_CONTROLS)?;
                    ctrl_entry |= VMENTRY_GUEST_IA32E;
                    vcpu.write_vmcs(VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry)?;
                } else if !long_mode && efer & EFER_LMA != 0 {
                    efer &= !EFER_LMA;
                    vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, efer)?;
                    let mut ctrl_entry = vcpu.read_vmcs(VMCS_CTRL_VMENTRY_CONTROLS)?;
                    ctrl_entry &= !VMENTRY_GUEST_IA32E;
                    vcpu.write_vmcs(VMCS_CTRL_VMENTRY_CONTROLS, ctrl_entry)?;
                }
            }
        }
        _ => {
            return Err(Error::Unhandled(
                VMX_REASON_MOV_CR,
                format!("access type {}: unimplemented", access_type),
            ));
        }
    }

    Ok(HandleResult::Next)
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_EPT_VIOLATION
////////////////////////////////////////////////////////////////////////////////

pub fn ept_read(qual: u64) -> bool {
    qual & 1 > 0
}

pub fn ept_write(qual: u64) -> bool {
    qual & 0b10 > 0
}

pub fn ept_instr_fetch(qual: u64) -> bool {
    qual & 0b100 > 0
}

pub fn ept_page_walk(qual: u64) -> bool {
    qual & (1 << 7) > 0 && qual & (1 << 8) == 0
}

pub fn handle_ept_violation(
    _vcpu: &VCPU,
    _gth: &mut GuestThread,
    _gpa: usize,
) -> Result<HandleResult, Error> {
    // we need to handle MMIOs. But for now we just resume the vm.
    Ok(HandleResult::Resume)
}
