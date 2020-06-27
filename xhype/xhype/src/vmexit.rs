#[allow(unused_imports)]
use super::consts::msr::*;
#[allow(unused_imports)]
use super::hv::vmx::*;
use super::paging::*;
#[allow(unused_imports)]
use super::x86::*;
use super::{Error, GuestThread, HandleResult, X86Reg, VCPU};
use crate::decode::emulate_mem_insn;
use crate::ioapic::ioapic_access;
use log::{error, info, trace, warn};
use std::mem::size_of;
// Fix me!
// this function is extremely unsafe. The purpose is to read from guest's memory,
// since the high memory address are the same as the host, we just directly read
// the host's memory. There should be better ways to implement this.
pub fn read_host_mem<T>(base: u64, index: u64) -> T {
    // println!("read from base = {:x}, index = {}", base, index);
    let ptr = (base + index * size_of::<T>() as u64) as *const T;
    unsafe { ptr.read() }
}

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

const ADDR_MASK: u64 = 0xffffffffffff;
pub fn simulate_paging(vcpu: &VCPU, addr_v: u64) -> Result<u64, Error> {
    let addr_v = ADDR_MASK & addr_v;
    // println!("addr_v = {:x}", addr_v);
    let cr0 = vcpu.read_reg(X86Reg::CR0)?;
    if cr0 & X86_CR0_PG == 0 {
        return Ok(addr_v);
    }
    let cr3 = vcpu.read_reg(X86Reg::CR3)?;
    // println!("cr3 = {:x}", cr3);
    let pml4e: u64 = read_host_mem(cr3 & !0xfff, pml4_index(addr_v));
    // println!("pml4e = {:x}", pml4e);
    if pml4e & PG_P == 0 {
        return Err("simulate_paging: page fault\n")?;
    }
    let pdpte: u64 = read_host_mem(pml4e & !0xfff, pdpt_index(addr_v));
    // println!("pdpte = {:x}", pdpte);
    if pdpte & PG_P == 0 {
        return Err("simulate_paging: page fault\n")?;
    } else if pdpte & PG_PS > 0 {
        return Ok((pdpte & !0x3fffffff) | (addr_v & 0x3fffffff));
    }
    let pde: u64 = read_host_mem(pdpte & !0xfff, pd_index(addr_v));
    // println!("pde = {:x}", pde);
    if pde & PG_P == 0 {
        return Err("simulate_paging: page fault\n")?;
    } else if pde & PG_PS > 0 {
        return Ok((pde & !0x1fffff) | (addr_v & 0x1fffff));
    }
    let pte: u64 = read_host_mem(pde, pt_index(addr_v));
    // println!("pte = {:x}", pte);
    if pte & PG_P == 0 {
        Err("simulate_paging: page fault\n")?
    } else {
        Ok((pte & !0xfff) | (addr_v & 0xfff))
    }
}

#[allow(dead_code)]
pub fn get_vmexit_instr(vcpu: &VCPU) -> Result<Vec<u8>, Error> {
    let len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
    let rip_v = vcpu.read_vmcs(VMCS_GUEST_RIP)?;
    let rip = simulate_paging(&vcpu, rip_v)?;
    Ok((0..len).map(|i| read_host_mem::<u8>(rip, i)).collect())
}

#[allow(dead_code)]
pub fn get_vmexit_instr_more(
    vcpu: &VCPU,
    _gth: &GuestThread,
    before: u64,
    after: u64,
) -> Result<[Vec<u8>; 3], Error> {
    let len = vcpu.read_vmcs(VMCS_RO_VMEXIT_INSTR_LEN)?;
    let rip_v = vcpu.read_vmcs(VMCS_GUEST_RIP)?;
    let rip = simulate_paging(&vcpu, rip_v)?;
    Ok([
        (0..before)
            .map(|i| read_host_mem::<u8>(rip - before, i))
            .collect(),
        (0..len).map(|i| read_host_mem::<u8>(rip, i)).collect(),
        (0..after)
            .map(|i| read_host_mem::<u8>(rip + len, i))
            .collect(),
    ])
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_MOV_CR
////////////////////////////////////////////////////////////////////////////////

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

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_RDMSR, VMX_REASON_WRMSR
////////////////////////////////////////////////////////////////////////////////

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
    msr: u32,
    read: bool,
    new_value: u64,
    _vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    if read {
        error!("read from unknown msr: {:08x} ", msr);
        Err(Error::Unhandled(VMX_REASON_RDMSR, "unknown msr"))
    } else {
        error!("write {:x} to unknown msr: {:08x} ", new_value, msr);
        Err(Error::Unhandled(VMX_REASON_WRMSR, "unknown msr"))
    }
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
        let r = match msr {
            MSR_MTRRCAP | MSR_MTRRDEF_TYPE | MSR_IA32_BIOS_SIGN_ID => 0,
            _ => unreachable!(),
        };
        write_msr_to_reg(r, vcpu)
    } else {
        warn!("write {:x} to read-only msr {:x}", new_value, msr);
        Ok(HandleResult::Next)
    }
}

fn emsr_pat(
    _msr: u32,
    read: bool,
    new_value: u64,
    vcpu: &VCPU,
    gth: &GuestThread,
) -> Result<HandleResult, Error> {
    // unimplemented!();
    if read {
        write_msr_to_reg(gth.msr_pat.get(), vcpu)
    } else {
        gth.msr_pat.set(new_value);
        Ok(HandleResult::Next)
    }
}

fn emsr_apicbase(
    _msr: u32,
    read: bool,
    new_value: u64,
    vcpu: &VCPU,
    gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let value = if gth.id == 0 {
        0xfee00d00 // BSP
    } else {
        0xfee00c00 // non BSP
    };
    if read {
        write_msr_to_reg(value, vcpu)
    } else {
        if new_value == value {
            Ok(HandleResult::Next)
        } else {
            Err(Error::Unhandled(
                VMX_REASON_WRMSR,
                "apic base cannot be changed",
            ))
        }
    }
}

macro_rules! arr {
    ($id: ident $name: ident: [$ty: ty; _] = $value: expr) => {
        $id $name: [$ty; $value.len()] = $value;
    }
}

arr!(static MSR_HANDLERS: [MSRHander; _] = [
    MSRHander(MSR_IA32_APICBASE, emsr_apicbase),
    MSRHander(MSR_IA32_CR_PAT, emsr_pat),
    MSRHander(MSR_MTRRDEF_TYPE, emsr_rdonly),
    MSRHander(MSR_MTRRCAP, emsr_rdonly),
    MSRHander(MSR_IA32_BIOS_SIGN_ID, emsr_rdonly),
    MSRHander(MSR_IA32_MISC_ENABLE, emsr_miscenable),
    MSRHander(MSR_LAPIC_ICR, emsr_unimpl),
    MSRHander(MSR_EFER, emsr_efer),
]);

pub fn handle_msr_access(
    read: bool,
    vcpu: &VCPU,
    gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let ecx = (vcpu.read_reg(X86Reg::RCX)? & 0xffffffff) as u32;
    let new_value = if !read {
        let rdx = vcpu.read_reg(X86Reg::RDX)?;
        let rax = vcpu.read_reg(X86Reg::RAX)?;
        let v = (rdx << 32) | rax;
        info!("write msr = {:08x}, new_value = {:x}", ecx, v);
        v
    } else {
        info!("read msr = {:08x}", ecx);
        0
    };
    for handler in MSR_HANDLERS.iter() {
        if handler.0 == ecx {
            return handler.1(ecx, read, new_value, vcpu, gth);
        }
    }
    emsr_unimpl(ecx, read, new_value, vcpu, gth)
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_IO
////////////////////////////////////////////////////////////////////////////////
fn io_size(qual: u64) -> u64 {
    (qual & 0b111) + 1 // Vol.3, table 27-5
}

fn io_in(qual: u64) -> bool {
    (qual >> 3) & 1 == 1
}

// fn io_str_instr(qual: u64) -> bool {
//     (qual >> 4) & 1 == 1
// }
// fn io_rep_prefixed(qual: u64) -> bool {
//     (qual >> 5) & 1 == 1
// }

// fn io_dx(qual: u64) -> bool {
//     qual >> 6 & 1 == 0
// }

fn io_port(qual: u64) -> u16 {
    (qual >> 16 & 0xffff) as u16
}

fn set_all_one(rax: u64, size: u64) -> u64 {
    rax | match size {
        1 => 0xff,
        2 => 0xffff,
        4 => 0xffffffff,
        _ => unreachable!(),
    }
}

fn set_all_zero(rax: u64, size: u64) -> u64 {
    rax & !match size {
        1 => 0xff,
        2 => 0xffff,
        4 => 0xffffffff,
        _ => unreachable!(),
    }
}

const CONFIG_DATA: u16 = 0xcfc;
const CONFIG_DATA2: u16 = 0xcfe; // fix me: what does 0xcfe mean?
const CONFIG_ADDRESS: u16 = 0xcf8;
fn cfg_address_handler(qual: u64, vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let cf8 = { gth.vm.read().unwrap().cf8 };
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let size = io_size(qual);
    let port = io_port(qual);
    let offset = cf8_offset(cf8);
    if cf8_bdf(cf8) == 0 {
        trace!(
            "in = {}, rax = {:x}, port = {:x}, offset = {:x}, size = {:x}",
            io_in(qual),
            rax,
            port,
            offset,
            size
        );
    }
    if cf8_enabled(cf8) {
        let bdf = cf8_bdf(cf8);
        if bdf == 0 {
            // only host bridge is supported
            if io_in(qual) {
                let mut v = gth.vm.read().unwrap().host_bridge_data[offset as usize >> 2];
                if size == 1 {
                    v >>= (port & 3) * 8;
                } else if size == 2 {
                    v >>= ((port & 2) >> 1) * 16;
                }
                info!(
                    "return size = {}, value = 0x{:0width$x} from port = {:x}",
                    size,
                    v,
                    port,
                    width = size as usize * 2
                );
                vcpu.write_reg(X86Reg::RAX, set_all_zero(rax, size) | v as u64)?;
            } else {
                if size == 4 {
                    gth.vm.write().unwrap().host_bridge_data[offset as usize >> 2] =
                        (rax & 0xffffffff) as u32;
                } else {
                    trace!(
                        "write data {:x} to port={:x}, offset={:x}",
                        rax,
                        port,
                        offset
                    );
                }
            }
        } else {
            if io_in(qual) {
                trace!("bdf = {:x}, return value = all one", bdf,);
                vcpu.write_reg(X86Reg::RAX, set_all_one(rax, size))?;
            }
        }
    } else {
        if io_in(qual) {
            vcpu.write_reg(X86Reg::RAX, set_all_one(rax, size))?;
        }
    }
    Ok(HandleResult::Next)
}

fn cf8_enabled(cf8: u32) -> bool {
    cf8 >> 31 > 0
}

fn cf8_offset(cf8: u32) -> u32 {
    cf8 & 0xff
}

// fn cf8_func(cf8: u32) -> u32 {
//     (cf8 >> 8) & 0b111
// }

// fn cf8_dev(cf8: u32) -> u32 {
//     (cf8 >> 11) & 0b11111
// }

fn cf8_bdf(cf8: u32) -> u16 {
    ((cf8 >> 8) & 0xffff) as u16
}

// fn cf8_bus(cf8: u32) -> u32 {
//     (cf8 >> 16) & 0xff
// }

fn cf8_handler(qual: u64, vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let size = io_size(qual);
    if size != 4 {
        if io_in(qual) {
            vcpu.write_reg(X86Reg::RAX, set_all_one(rax, size))?;
        }
    }
    if io_in(qual) {
        let cf8_value = gth.vm.read().unwrap().cf8;
        vcpu.write_reg(X86Reg::RAX, set_all_zero(rax, size) | cf8_value as u64)?;
    } else {
        if cf8_bdf(rax as u32) == 0 {
            info!(
                "set cf8 to bdf = {:x}, offset = {:x}",
                cf8_bdf(rax as u32),
                cf8_offset(rax as u32)
            );
        }
        gth.vm.write().unwrap().cf8 = rax as u32;
    }
    Ok(HandleResult::Next)
}

pub fn unknown_port_handler(
    qual: u64,
    vcpu: &VCPU,
    _gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let rax = vcpu.read_reg(X86Reg::RAX)?;
    let port = io_port(qual);
    if io_in(qual) {
        error!("write to io port = {:x}, rax={:x}", port, rax);
    } else {
        error!("write to io port = {:x}, rax={:x}", port, rax);
    }
    Err(Error::Unhandled(VMX_REASON_IO, "unknown port"))
}

pub fn handle_io(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let qual = vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?;
    let edx = (vcpu.read_reg(X86Reg::RDX)? & 0xffff) as u16;
    match edx {
        CONFIG_DATA | CONFIG_DATA2 => cfg_address_handler(qual, vcpu, gth),
        CONFIG_ADDRESS => cf8_handler(qual, vcpu, gth),
        _ => unknown_port_handler(qual, vcpu, gth),
    }
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_VMCALL
////////////////////////////////////////////////////////////////////////////////
pub fn default_vmcall_handler(vcpu: &VCPU, _gth: &GuestThread) -> Result<HandleResult, Error> {
    let num = vcpu.read_reg(X86Reg::RDI)?;
    let vmcall_args = vcpu.read_reg(X86Reg::RSI)?;
    match num {
        0 => return Ok(HandleResult::Exit),
        1 => {
            let string = unsafe {
                let ptr = vmcall_args as *const &str;
                ptr.read()
            };
            println!("{}", string);
        }
        _ => {}
    };
    Ok(HandleResult::Next)
}

pub fn handle_vmcall(vcpu: &VCPU, gth: &GuestThread) -> Result<HandleResult, Error> {
    let handler = { gth.vm.read().unwrap().vmcall_hander };
    handler(vcpu, gth)
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_EPT_VIOLATION
////////////////////////////////////////////////////////////////////////////////

fn ept_read(qual: u64) -> bool {
    qual & 1 > 0
}

fn ept_write(qual: u64) -> bool {
    qual & 0b10 > 0
}

fn ept_instr_fetch(qual: u64) -> bool {
    qual & 0b100 > 0
}

pub fn handle_ept_violation(
    gpa: usize,
    vcpu: &VCPU,
    gth: &GuestThread,
) -> Result<HandleResult, Error> {
    let qual = vcpu.read_vmcs(VMCS_RO_EXIT_QUALIFIC)?;
    trace!(
        "ept, read = {}, write = {}, fetch = {}",
        ept_read(qual),
        ept_write(qual),
        ept_instr_fetch(qual)
    );
    if gpa >= IO_APIC_BASE && gpa < IO_APIC_BASE + PAGE_SIZE {
        let insn = get_vmexit_instr(vcpu)?;
        emulate_mem_insn(vcpu, gth, &insn, ioapic_access, gpa)?;
        Ok(HandleResult::Next)
    } else {
        Ok(HandleResult::Resume)
    }
}
