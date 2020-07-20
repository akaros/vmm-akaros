/* SPDX-License-Identifier: GPL-2.0-only */

use crate::consts::msr::*;
use crate::consts::x86::*;
use crate::cpuid::do_cpuid;
use crate::err::Error;
use crate::hv::vmx::*;
use crate::hv::X86Reg;
use crate::hv::{vmx_read_capability, VMXCap};
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
// VMX_REASON_RDMSR, VMX_REASON_WRMSR
////////////////////////////////////////////////////////////////////////////////
#[inline]
fn write_msr_to_reg(msr_value: u64, vcpu: &VCPU) -> Result<(), Error> {
    let new_eax = msr_value & 0xffffffff;
    let new_edx = msr_value >> 32;
    vcpu.write_reg(X86Reg::RAX, new_eax)?;
    vcpu.write_reg(X86Reg::RDX, new_edx)
}

// TODO: A real CPU will generate #GP if an unknown MSR is accessed.
fn msr_unknown(
    _vcpu: &VCPU,
    _gth: &GuestThread,
    new_value: Option<u64>,
    msr: u32,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        let err_msg = format!("guest writes {:x} to unknown msr: {:08x}", v, msr);
        Err(Error::Unhandled(VMX_REASON_WRMSR, err_msg))
    } else {
        let err_msg = format!("guest reads from unknown msr: {:08x}", msr);
        Err(Error::Unhandled(VMX_REASON_RDMSR, err_msg))
    }
}

fn msr_efer(
    vcpu: &VCPU,
    _gth: &GuestThread,
    new_value: Option<u64>,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, v)?;
    } else {
        let efer = vcpu.read_vmcs(VMCS_GUEST_IA32_EFER)?;
        write_msr_to_reg(efer, vcpu)?;
    }
    Ok(HandleResult::Next)
}

pub fn handle_msr_access(
    vcpu: &VCPU,
    gth: &GuestThread,
    read: bool,
) -> Result<HandleResult, Error> {
    let msr = (vcpu.read_reg(X86Reg::RCX)? & 0xffffffff) as u32;
    let new_value = if read {
        None
    } else {
        let edx = vcpu.read_reg(X86Reg::RDX)? & 0xffffffff;
        let eax = vcpu.read_reg(X86Reg::RAX)? & 0xffffffff;
        Some((edx << 32) | eax)
    };
    match msr {
        MSR_EFER => msr_efer(vcpu, gth, new_value),
        _ => msr_unknown(vcpu, gth, new_value, msr),
    }
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_IO
////////////////////////////////////////////////////////////////////////////////

struct ExitQualIO(u64);

impl ExitQualIO {
    pub fn size(&self) -> u64 {
        (self.0 & 0b111) + 1 // Vol.3, table 27-5
    }

    pub fn is_in(&self) -> bool {
        (self.0 >> 3) & 1 == 1
    }

    pub fn port(&self) -> u16 {
        ((self.0 >> 16) & 0xffff) as u16
    }
}

const COM1_BASE: u16 = 0x3f8;
const COM1_MAX: u16 = 0x3ff;

pub fn handle_io(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let qual = ExitQualIO(vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?);
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let port = qual.port();
    match port {
        COM1_BASE..=COM1_MAX => {
            if qual.is_in() {
                let v = gth.vm.com1.write().unwrap().read(port - COM1_BASE);
                // fixme: handle partial registers
                vcpu.write_reg(X86Reg::RAX, v as u64)?;
            } else {
                let v = (rax & 0xff) as u8;
                gth.vm.com1.write().unwrap().write(port - COM1_BASE, v);
            }
        }
        _ => return Err((VMX_REASON_IO, format!("cannot handle IO port 0x{:x}", port)))?,
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

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_CPUID
////////////////////////////////////////////////////////////////////////////////

// the implementation of handle_cpuid() is inspired by handle_vmexit_cpuid()
// from Akaros/kern/arch/x86/trap.c
pub fn handle_cpuid(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let eax_in = (vcpu.read_reg(X86Reg::RAX)? & 0xffffffff) as u32;
    let ecx_in = (vcpu.read_reg(X86Reg::RCX)? & 0xffffffff) as u32;

    let (mut eax, mut ebx, mut ecx, mut edx) = do_cpuid(eax_in, ecx_in);
    match eax_in {
        0x1 => {
            // Set the guest thread id into the apic ID field in CPUID.
            ebx &= 0x0000ffff;
            ebx |= (gth.vm.cores & 0xff) << 16;
            ebx |= (gth.id & 0xff) << 24;

            // Set the hypervisor bit to let the guest know it is virtualized
            ecx |= CPUID_HV;

            // unset monitor capability, vmx capability, and perf capability
            ecx &= !(CPUID_MONITOR | CPUID_VMX | CPUID_PDCM);

            // unset osxsave if it is not supported or it is not turned on
            if ecx & CPUID_XSAVE == 0 || vcpu.read_reg(X86Reg::CR4)? & X86_CR4_OSXSAVE == 0 {
                ecx &= !CPUID_OSXSAVE;
            } else {
                ecx |= CPUID_OSXSAVE;
            }
        }
        0x7 => {
            // Do not advertise TSC_ADJUST
            ebx &= !CPUID_TSC_ADJUST;
        }
        0xa => {
            eax = 0;
            ebx = 0;
            ecx = 0;
            edx = 0;
        }
        0x4000_0000 => {
            // eax indicates the highest eax in Hypervisor leaf
            // https://www.kernel.org/doc/html/latest/virt/kvm/cpuid.html
            eax = 0x4000_0000;
        }
        _ => {}
    }
    vcpu.write_reg(X86Reg::RAX, eax as u64)?;
    vcpu.write_reg(X86Reg::RBX, ebx as u64)?;
    vcpu.write_reg(X86Reg::RCX, ecx as u64)?;
    vcpu.write_reg(X86Reg::RDX, edx as u64)?;
    info!(
        "cpuid, in: eax={:x}, ecx={:x}; out: eax={:x}, ebx={:x}, ecx={:x}, edx={:x}",
        eax_in, ecx_in, eax, ebx, ecx, edx
    );
    Ok(HandleResult::Next)
}
