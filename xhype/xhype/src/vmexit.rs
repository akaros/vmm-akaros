/* SPDX-License-Identifier: GPL-2.0-only */

use crate::apic::apic_access;
use crate::consts::msr::*;
use crate::consts::x86::*;
use crate::cpuid::do_cpuid;
use crate::decode::emulate_mem_insn;
use crate::err::Error;
use crate::hv::vmx::*;
use crate::hv::X86Reg;
use crate::hv::{vmx_read_capability, VMXCap};
use crate::ioapic::ioapic_access;
use crate::virtio::mmio::virtio_mmio;
use crate::{GuestThread, MsrPolicy, PolicyList, PortPolicy, VCPU};
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

const ADDR_MASK: u64 = 0xffffffffffff;

fn pt_index(addr: u64) -> u64 {
    (addr >> 12) & 0x1ff
}

fn pd_index(addr: u64) -> u64 {
    (addr >> 21) & 0x1ff
}

fn pdpt_index(addr: u64) -> u64 {
    (addr >> 30) & 0x1ff
}

fn pml4_index(addr: u64) -> u64 {
    (addr >> 39) & 0x1ff
}

/// only supports 4-level paging
pub(crate) unsafe fn emulate_paging(
    vcpu: &VCPU,
    gth: &GuestThread,
    linear_addr: u64,
) -> Result<u64, Error> {
    let cr0 = vcpu.read_reg(X86Reg::CR0)?;
    if cr0 & X86_CR0_PG == 0 {
        return Ok(linear_addr);
    }
    let cr3 = vcpu.read_reg(X86Reg::CR3)?;
    trace!("linear address = {:x}, cr3 = {:x}", linear_addr, cr3);
    trace!("linear address components: pml4_index = {:x}, pdpt_index = {:x}, pd_index = {:x}, pt_index = {:x}", pml4_index(linear_addr), pdpt_index(linear_addr), pd_index(linear_addr), pt_index(linear_addr));
    for i in pml4_index(linear_addr).checked_sub(5).unwrap_or(0)
        ..std::cmp::min(pml4_index(linear_addr) + 5, 512)
    {
        trace!(
            "pml4 entry {} = {:x}",
            i,
            *gth.vm.read_guest_mem::<u64>((cr3 & !0xfff) & ADDR_MASK, i)
        )
    }
    let pml4e: u64 = *gth
        .vm
        .read_guest_mem((cr3 & !0xfff) & ADDR_MASK, pml4_index(linear_addr));
    trace!("pml4e = {:x}", pml4e);
    if pml4e & PG_P == 0 {
        return Err("emulate_paging: page fault at pml4e".to_string())?;
    }
    for i in pdpt_index(linear_addr).checked_sub(5).unwrap_or(0)
        ..std::cmp::min(pdpt_index(linear_addr) + 5, 512)
    {
        trace!(
            "pdpt entry {} = {:x}",
            i,
            *gth.vm
                .read_guest_mem::<u64>((pml4e & !0xfff) & ADDR_MASK, i)
        )
    }
    let pdpte: u64 = *gth
        .vm
        .read_guest_mem((pml4e & !0xfff) & ADDR_MASK, pdpt_index(linear_addr));
    trace!("pdpte = {:x}", pdpte);
    if pdpte & PG_P == 0 {
        return Err("emulate_paging: page fault at pdpte".to_string())?;
    } else if pdpte & PG_PS > 0 {
        return Ok((pdpte & !0x3fffffff) | (linear_addr & 0x3fffffff));
    }
    for i in pd_index(linear_addr).checked_sub(5).unwrap_or(0)
        ..std::cmp::min(pd_index(linear_addr) + 5, 512)
    {
        trace!(
            "pd entry {} = {:x}",
            i,
            *gth.vm
                .read_guest_mem::<u64>((pdpte & !0xfff) & ADDR_MASK, i)
        )
    }
    let pde: u64 = *gth
        .vm
        .read_guest_mem((pdpte & !0xfff) & ADDR_MASK, pd_index(linear_addr));
    trace!("pde = {:x}", pde);
    if pde & PG_P == 0 {
        return Err("emulate_paging: page fault at pde".to_string())?;
    } else if pde & PG_PS > 0 {
        return Ok((pde & !0x1fffff) | (linear_addr & 0x1fffff));
    }
    for i in pt_index(linear_addr).checked_sub(5).unwrap_or(0)
        ..std::cmp::min(pt_index(linear_addr) + 5, 512)
    {
        trace!(
            "pt entry {} = {:x}",
            i,
            *gth.vm.read_guest_mem::<u64>((pde & !0xfff) & ADDR_MASK, i)
        )
    }
    let pte: u64 = *gth
        .vm
        .read_guest_mem((pde & !0xfff) & ADDR_MASK, pt_index(linear_addr));
    trace!("pte = {:x}", pte);
    if pte & PG_P == 0 {
        return Err("emulate_paging: page fault at pte".to_string())?;
    } else {
        Ok(((pte & !0xfff) | (linear_addr & 0xfff)) & ADDR_MASK)
    }
}

pub fn get_vmexit_instr(vcpu: &VCPU, gth: &GuestThread) -> Result<Vec<u8>, Error> {
    let len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
    let rip_v = vcpu.read_vmcs(VMCS_GUEST_RIP)?;
    let rip_gpa = unsafe { emulate_paging(vcpu, gth, rip_v)? };
    Ok((0..len)
        .map(|i| unsafe { *gth.vm.read_guest_mem::<u8>(rip_gpa, i) })
        .collect())
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
                X86Reg::CR4 => {
                    new_value |= X86_CR4_VMXE;
                    vcpu.write_vmcs(VMCS_CTRL_CR4_SHADOW, new_value & !X86_CR4_VMXE)?;
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
                    // to do: check more segment registers according to
                    // 26.3.1.2 Checks on Guest Segment Registers
                    vcpu.write_vmcs(VMCS_GUEST_TR_AR, 0x8b)?;
                    warn!("long mode is turned on");
                    vcpu.dump().unwrap();
                } else if !long_mode && efer & EFER_LMA != 0 {
                    warn!("long mode turned off");
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

// Table 24-14. Format of the VM-Entry Interruption-Information Field
pub fn inject_exception(
    vcpu: &VCPU,
    vector: u8,
    exception_type: u8,
    code: Option<u32>,
) -> Result<(), Error> {
    if let Some(code) = code {
        vcpu.write_vmcs(VMCS_CTRL_VMENTRY_EXC_ERROR, code as u64)?;
        let info = 1 << 31 | vector as u64 | (exception_type as u64) << 8 | 1 << 11;
        vcpu.write_vmcs(VMCS_CTRL_VMENTRY_IRQ_INFO, info)?;
    } else {
        let info = 1 << 31 | vector as u64 | (exception_type as u64) << 8;
        vcpu.write_vmcs(VMCS_CTRL_VMENTRY_IRQ_INFO, info)?;
    }
    Ok(())
}

fn msr_apply_policy(
    vcpu: &VCPU,
    gth: &GuestThread,
    new_value: Option<u64>,
    msr: u32,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        warn!("guest writes {:x} to unknown msr: {:08x}", v, msr);
    }
    match gth.vm.msr_policy {
        MsrPolicy::GP => {
            inject_exception(vcpu, 13, 3, Some(0))?;
            Ok(HandleResult::Resume)
        }
        MsrPolicy::AllOne => {
            if new_value.is_none() {
                warn!(
                    "guest reads from unknown msr: {:08x}, return 0x{:x}",
                    msr,
                    u64::MAX
                );
                write_msr_to_reg(u64::MAX, vcpu)?;
            }
            Ok(HandleResult::Next)
        }
        MsrPolicy::Random => {
            if new_value.is_none() {
                let v = rand::random();
                warn!(
                    "guest reads from unknown msr: {:08x}, return 0x{:x}",
                    msr, v
                );
                write_msr_to_reg(v, vcpu)?;
            }
            Ok(HandleResult::Next)
        }
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

fn msr_apicbase(
    vcpu: &VCPU,
    gth: &mut GuestThread,
    new_value: Option<u64>,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        if v != gth.apic.msr_apic_base {
            // to do: handle apic_base change
            error!("guest changes MSR APIC_BASE");
            gth.apic.msr_apic_base = v
        }
    } else {
        write_msr_to_reg(gth.apic.msr_apic_base, vcpu)?;
    }
    Ok(HandleResult::Next)
}

fn msr_read_only(
    vcpu: &VCPU,
    _gth: &GuestThread,
    new_value: Option<u64>,
    msr: u32,
    default_value: u64,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        if v != default_value {
            warn!(
                "guest writes 0x{:x} to msr 0x{:x}, different from default 0x{:x}",
                v, msr, default_value
            );
        }
    } else {
        write_msr_to_reg(default_value, vcpu)?;
    }
    Ok(HandleResult::Next)
}

fn msr_pat(
    vcpu: &VCPU,
    gth: &mut GuestThread,
    new_value: Option<u64>,
) -> Result<HandleResult, Error> {
    if let Some(v) = new_value {
        gth.pat_msr = v;
    } else {
        write_msr_to_reg(gth.pat_msr, vcpu)?;
    }
    Ok(HandleResult::Next)
}

pub fn handle_msr_access(
    vcpu: &VCPU,
    gth: &mut GuestThread,
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
        MSR_IA32_MISC_ENABLE => {
            // enable fast string, disable pebs and bts.
            let misc_enable = 1 | ((1 << 12) | (1 << 11));
            msr_read_only(vcpu, gth, new_value, msr, misc_enable)
        }
        MSR_IA32_BIOS_SIGN_ID => msr_read_only(vcpu, gth, new_value, msr, 0),
        MSR_IA32_CR_PAT => msr_pat(vcpu, gth, new_value),
        MSR_IA32_APIC_BASE => msr_apicbase(vcpu, gth, new_value),
        _ => match &gth.vm.msr_list {
            PolicyList::Apply(set) => {
                if set.contains(&msr) {
                    msr_apply_policy(vcpu, gth, new_value, msr)
                } else {
                    msr_unknown(vcpu, gth, new_value, msr)
                }
            }
            PolicyList::Except(set) => {
                if !set.contains(&msr) {
                    msr_apply_policy(vcpu, gth, new_value, msr)
                } else {
                    msr_unknown(vcpu, gth, new_value, msr)
                }
            }
        },
    }
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_IO
////////////////////////////////////////////////////////////////////////////////

struct ExitQualIO(u64);

impl ExitQualIO {
    pub fn size(&self) -> u8 {
        ((self.0 & 0b111) + 1) as u8 // Vol.3, table 27-5
    }

    pub fn is_in(&self) -> bool {
        (self.0 >> 3) & 1 == 1
    }

    pub fn port(&self) -> u16 {
        ((self.0 >> 16) & 0xffff) as u16
    }
}

const PCI_CONFIG_ADDR: u16 = 0xcf8;
const PCI_CONFIG_DATA: u16 = 0xcfc;
const PCI_CONFIG_DATA_MAX: u16 = 0xcff;

const COM1_BASE: u16 = 0x3f8;
const COM1_MAX: u16 = 0x3ff;

const RTC_PORT_REG: u16 = 0x70;
const RTC_PORT_DATA: u16 = 0x71;

fn port_apply_policy(
    vcpu: &VCPU,
    gth: &GuestThread,
    data_out: Option<u64>,
    size: u8,
    port: u16,
) -> Result<(), Error> {
    if let Some(v) = data_out {
        warn!("guest writes 0x{:x?} to unknown port: 0x{:x}", v, port);
    } else {
        let ret = match gth.vm.port_policy {
            PortPolicy::AllOne => u64::MAX,
            PortPolicy::Random => rand::random(),
        };
        match size {
            1 => vcpu.write_reg_16_low(X86Reg::RAX, (ret & 0xff) as u8)?,
            2 => vcpu.write_reg_16(X86Reg::RAX, (ret & 0xffff) as u16)?,
            4 => vcpu.write_reg(X86Reg::RAX, ret & 0xffffffff)?,
            _ => unreachable!(),
        }
        warn!(
            "guest reads from unknown port: 0x{:x}, return 0x{:x}",
            port, ret
        );
    }
    Ok(())
}

fn port_unknown(
    _vcpu: &VCPU,
    _gth: &GuestThread,
    data_out: Option<u64>,
    _size: u8,
    port: u16,
) -> Result<HandleResult, Error> {
    let err_msg = if let Some(v) = data_out {
        format!("guest writes 0x{:x?} to unknown port: 0x{:x}", v, port)
    } else {
        format!("guest reads from unknown port: 0x{:x}", port)
    };
    error!("{}", &err_msg);
    Err(Error::Unhandled(VMX_REASON_IO, err_msg))
}

pub fn handle_io(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let qual = ExitQualIO(vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?);
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let port = qual.port();
    let size = qual.size();
    match port {
        RTC_PORT_REG => {
            if qual.is_in() {
                vcpu.write_reg_16_low(X86Reg::RAX, gth.vm.rtc.read().unwrap().reg)?;
            } else {
                gth.vm.rtc.write().unwrap().reg = (rax & 0xff) as u8;
            }
        }
        RTC_PORT_DATA => {
            if qual.is_in() {
                let v = gth.vm.rtc.read().unwrap().read();
                vcpu.write_reg_16_low(X86Reg::RAX, v)?;
            } else {
                error!("guest writes {:x} to RTC port {:x}", rax, RTC_PORT_DATA);
            }
        }
        COM1_BASE..=COM1_MAX => {
            if qual.is_in() {
                let v = gth.vm.com1.lock().unwrap().read(port - COM1_BASE);
                vcpu.write_reg_16_low(X86Reg::RAX, v)?;
            } else {
                let v = (rax & 0xff) as u8;
                gth.vm.com1.lock().unwrap().write(port - COM1_BASE, v);
            }
        }
        PCI_CONFIG_ADDR => {
            if qual.is_in() {
                let v = gth.vm.pci_bus.lock().unwrap().config_addr.0;
                vcpu.write_reg(X86Reg::RAX, v as u64)?;
            } else {
                let v = (rax & 0xffffffff) as u32;
                gth.vm.pci_bus.lock().unwrap().config_addr.0 = v;
            }
        }
        PCI_CONFIG_DATA..=PCI_CONFIG_DATA_MAX => {
            if qual.is_in() {
                let mut v = gth.vm.pci_bus.lock().unwrap().read();
                if size == 1 {
                    v >>= (port & 0b11) * 8;
                    vcpu.write_reg_16_low(X86Reg::RAX, (v & 0xff) as u8)?;
                } else if size == 2 {
                    v >>= (port & 0b10) * 8;
                    vcpu.write_reg_16(X86Reg::RAX, (v & 0xffff) as u16)?;
                } else {
                    vcpu.write_reg(X86Reg::RAX, v as u64)?;
                }
            } else {
                if size == 4 {
                    let mut pic_bus = gth.vm.pci_bus.lock().unwrap();
                    pic_bus.write((rax & 0xffffffff) as u32);
                } else {
                    // to do:
                    error!("guest writes non-4-byte data to pci, data = {:x}, size = {}, port = {:x}, current cf8 = {:x}", rax, size, port, gth.vm.pci_bus.lock().unwrap().config_addr.0);
                }
            }
        }
        _ => {
            let data_out = if qual.is_in() {
                None
            } else {
                match size {
                    1 => Some(rax & 0xff),
                    2 => Some(rax & 0xffff),
                    4 => Some(rax & 0xffffffff),
                    _ => unreachable!(),
                }
            };
            match &gth.vm.port_list {
                PolicyList::Apply(set) => {
                    if set.contains(&port) {
                        port_apply_policy(vcpu, gth, data_out, size, port)?;
                    } else {
                        port_unknown(vcpu, gth, data_out, size, port)?;
                    }
                }
                PolicyList::Except(set) => {
                    if !set.contains(&port) {
                        port_apply_policy(vcpu, gth, data_out, size, port)?;
                    } else {
                        port_unknown(vcpu, gth, data_out, size, port)?;
                    }
                }
            }
        }
    }
    Ok(HandleResult::Next)
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_EPT_VIOLATION
////////////////////////////////////////////////////////////////////////////////

pub fn ept_qual_description(qual: u64) -> String {
    let mut description = format!(
        "qual={:x}, read = {}, write = {}, instruction fetch = {}. valid = {}, page_walk = {}",
        qual,
        ept_read(qual),
        ept_write(qual),
        ept_instr_fetch(qual),
        ept_valid(qual),
        ept_page_walk(qual)
    );

    description
    // format!("qual={:x}, read = {}, write = {}, instruction fetch = {}, valid = ")
}

pub fn ept_valid(qual: u64) -> bool {
    qual & (1 << 7) > 0
}

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
    vcpu: &VCPU,
    gth: &mut GuestThread,
    gpa: usize,
) -> Result<HandleResult, Error> {
    let apic_base = gth.apic.msr_apic_base as usize & !0xfff;
    if (gpa & !0xfff) == apic_base {
        let insn = get_vmexit_instr(vcpu, gth)?;
        emulate_mem_insn(vcpu, gth, &insn, apic_access, gpa).unwrap();
        return Ok(HandleResult::Next);
    } else if (gpa & !0xfff) == IO_APIC_BASE {
        let insn = get_vmexit_instr(vcpu, gth)?;
        emulate_mem_insn(vcpu, gth, &insn, ioapic_access, gpa)?;
        return Ok(HandleResult::Next);
    } else {
        let virtio_start = gth.vm.virtio_base;
        if gpa >= virtio_start && gpa - virtio_start < PAGE_SIZE * gth.vm.virtio_mmio_devices.len()
        {
            let insn = get_vmexit_instr(vcpu, gth)?;
            let r = emulate_mem_insn(vcpu, gth, &insn, virtio_mmio, gpa);
            if r.is_err() {
                vcpu.dump()?;
                return Err(r.unwrap_err());
            }
            return Ok(HandleResult::Next);
        }
    }
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

            // unset monitor capability, vmx capability, perf capability,
            // and tsc deadline
            ecx &= !(CPUID_MONITOR | CPUID_VMX | CPUID_PDCM | CPUID_TSC_DL);

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
        0xd => {
            if (vmx_read_capability(VMXCap::CPU2)? >> 32) & CPU_BASED2_XSAVES_XRSTORS == 0 {
                eax = 0;
                ebx = 0;
                ecx = 0;
                edx = 0;
            }
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
