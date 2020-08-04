/* SPDX-License-Identifier: GPL-2.0-only */

/*!
Emulates a local APIC (Advanced Programmable Interrupt Controller)
*/

// to do: NMI, Spurious-Interrupt Vector

use crate::consts::msr::*;
use crate::consts::x86::*;
use crate::cpuid::do_cpuid;
use crate::err::Error;
use crate::hv::vmx::*;
use crate::hv::X86Reg;
use crate::mach::MachVMBlock;
use crate::utils::{get_tsc_frequency, mach_abs_time, mach_timebase_factor};
use crate::{GuestThread, VCPU};
#[allow(unused_imports)]
use log::*;

const OFFSET_ID: usize = 0x20;
const OFFSET_VER: usize = 0x30;
const OFFSET_TPR: usize = 0x80;
const OFFSET_APR: usize = 0x90;
const OFFSET_PPR: usize = 0xa0;
const OFFSET_EOI: usize = 0xb0;
const OFFSET_RRD: usize = 0xc0; // Remote Read Register?
const OFFSET_LDR: usize = 0xd0; // Logical Destination Register (LDR)
const OFFSET_DFR: usize = 0xe0; // Destination Format Register, xapic only
const OFFSET_SVR: usize = 0xf0;
const OFFSET_ISR: usize = 0x100;
const OFFSET_TMR: usize = 0x180;
const OFFSET_IRR: usize = 0x200;
const OFFSET_ESR: usize = 0x280;
const OFFSET_LVT_CMCI: usize = 0x2f0;
const OFFSET_ICR0: usize = 0x300; // inter-processor interrupt
const OFFSET_ICR32: usize = 0x310; //xapic only
const OFFSET_LVT_TIMER: usize = 0x320;
const OFFSET_LVT_THERMAL: usize = 0x330;
const OFFSET_LVT_PERF: usize = 0x340;
const OFFSET_LVT_LINT0: usize = 0x350;
const OFFSET_LVT_LINT1: usize = 0x360;
const OFFSET_LVT_ERROR: usize = 0x370;
const OFFSET_INIT_COUNT: usize = 0x380;
const OFFSET_CURR_COUNT: usize = 0x390;
const OFFSET_DCR: usize = 0x3e0; //Divide Configuration Register
const OFFSET_SELF_IPI: usize = 0x3f0; // x2apic only

const APIC_VER: u32 = 0x10;

const BIT_LVT_MASKED: u32 = 0x10000;
const TIMER_ONE_SHOT: u32 = 0b00;
const TIMER_PERIODIC: u32 = 0b01;
const TIMER_TCS_DDL: u32 = 0b10;

fn lvt_masked(lvt: u32) -> bool {
    lvt & BIT_LVT_MASKED > 0
}

fn lvt_idle(lvt: u32) -> bool {
    (lvt >> 12) & 1 == 0
}

fn lvt_vec(lvt: u32) -> u8 {
    (lvt & 0xff) as u8
}

fn lvt_timer_mode(lvt: u32) -> u32 {
    (lvt >> 17) & 0b11
}

fn priority(v: u8) -> u8 {
    v >> 4
}

// get the offset of the corresponding isr for a vector
fn isr_offset_vec(vector: u8) -> usize {
    OFFSET_ISR + (vector as usize / 32) * 0x10
}

fn irr_offset_vec(vector: u8) -> usize {
    OFFSET_IRR + (vector as usize / 32) * 0x10
}

fn tmr_offset_vec(vector: u8) -> usize {
    OFFSET_TMR + (vector as usize / 32) * 0x10
}

/**
    Represents a local APIC

    Apple's Hypervisor Framework does not support APIC virtualization. One
    possible reason is Apple actually hides this feature from framework users,
    because Apple needs this feature to implement scheduling. To be more specific,
    A VCPU of this framework is mapped to a pthread and it is scheduled by the macOS
    kernel. Therefore the macOS kernel needs to send interrupts to the VM such that
    a physical CPU could exit from VMX non-root mode and be used by the macOS kernel
    to execute other threads. So the APIC virtualization is already used by the
    framework and we have no way but to emulate the APIC by ourselves.

    The current implementation of APIC timer is based on VMX Preemption timer.
    See Intel SDM 25.5.1. When a guest thread starts, its local APIC's `next_timer`
    is first examined. If it is `Some(dead_line)`, then `vcpu.run_until(dead_line)` is called and when
    the `dead_line` reached, a vm-exit of`VMX_REASON_VMX_TIMER_EXPIRED` is generated and
    now we know we should fire a timer interrupt to the VCPU. See the comments in `fn run_on_inner`
    of `struct GuestThread` in `lib.rs`.
*/
pub struct Apic {
    /// apic id
    pub id: u32,
    /// the value stored in the MSR IA32_APIC_BASE, see Intel Manual Vol.3 Figure 10-26.
    pub msr_apic_base: u64,
    /// a virtual memory page of the host that is mapped to the guest's apic page, which
    /// is usually located at physical memory address 0xfee00000 of the guest.
    pub apic_page: MachVMBlock,
    pending_err: u32,
    frequency: u64,
    /// The time when the next APIC timer interrupt should be fired. This time is
    /// measured in mach_absolute_time unit, see Apple's [doc](https://developer.apple.com/documentation/kernel/1462446-mach_absolute_time). `None` represents the apic timer is disabled.
    pub next_timer: Option<u64>,
    timer_period: u64,
    isr_vec: Vec<u8>,
    // mach_timebase_info
    numer: u32,
    denom: u32,
}

impl Apic {
    pub fn new(base: u64, enabled: bool, x2mode: bool, id: u32, bsp: bool) -> Self {
        debug_assert_eq!(x2mode, false);
        let (eax, ebx, _, _) = do_cpuid(0x15, 0);
        let tsc_freq = get_tsc_frequency();
        let crystal_freq = tsc_freq * eax as u64 / ebx as u64;
        info!(
            "tsc = {}, eax = {}, ebx = {}, set apic frequency to {}",
            tsc_freq, eax, ebx, crystal_freq
        );
        let apic_page = MachVMBlock::new(PAGE_SIZE).unwrap();
        let msr_apic_base =
            base | if enabled {
                MSR_IA32_APIC_BASE_ENABLED
            } else {
                0
            } | if x2mode { MSR_IA32_APIC_BASE_X2 } else { 0 }
                | if bsp { MSR_IA32_APIC_BASE_BSP } else { 0 };
        let (numer, denom) = mach_timebase_factor().expect("cannot get mach_timebase_info");
        let mut apic = Apic {
            msr_apic_base,
            apic_page,
            id,
            pending_err: 0,
            frequency: crystal_freq,
            next_timer: None,
            timer_period: 0,
            isr_vec: Vec::new(),
            numer,
            denom,
        };
        apic.reset();
        apic
    }

    pub fn x2mode(&self) -> bool {
        self.msr_apic_base & (1 << 10) > 0
    }

    fn ns_to_abs(&self, ns: u64) -> u64 {
        ns * self.denom as u64 / self.numer as u64
    }

    fn clear_irr(&mut self, vector: u8) {
        let offset_irr = irr_offset_vec(vector);
        let irr: u32 = self.apic_page.read(offset_irr, 0);
        let vector_bit = 1 << (vector % 32);
        debug_assert_eq!(irr & vector_bit, vector_bit);
        self.apic_page.write(irr & !vector_bit, offset_irr, 0);
    }

    fn set_irr(&mut self, vector: u8) {
        let offset_irr = irr_offset_vec(vector);
        let irr: u32 = self.apic_page.read(offset_irr, 0);
        let vector_bit = 1 << (vector % 32);
        if irr & vector_bit == vector_bit {
            info!("vector {} discarded", vector);
        }
        self.apic_page.write(irr | vector_bit, offset_irr, 0);
    }

    fn clear_isr(&mut self, vector: u8) {
        let offset_isr = isr_offset_vec(vector);
        let isr: u32 = self.apic_page.read(offset_isr, 0);
        let vector_bit = 1 << (vector % 32);
        debug_assert_eq!(isr & vector_bit, vector_bit);
        self.apic_page.write(isr & !vector_bit, offset_isr, 0);
    }

    fn set_isr(&mut self, vector: u8) {
        let offset_isr = isr_offset_vec(vector);
        let isr: u32 = self.apic_page.read(offset_isr, 0);
        let vector_bit = 1 << (vector % 32);
        debug_assert_eq!(isr & vector_bit, 0);
        self.apic_page.write(isr | vector_bit, offset_isr, 0);
    }

    fn tmr_is_set(&self, vector: u8) -> bool {
        let offset_tmr = tmr_offset_vec(vector);
        let tmr: u32 = self.apic_page.read(offset_tmr, 0);
        let vector_bit = 1 << (vector % 32);
        tmr & vector_bit == vector_bit
    }

    pub fn read(&self, offset: usize) -> u64 {
        // to do: add more checks
        if offset == OFFSET_CURR_COUNT {
            unimplemented!("OFFSET_CURR_COUNT");
        };
        let mut result = self.apic_page.read::<u32>(offset, 0) as u64;
        if offset == OFFSET_ICR0 && self.x2mode() {
            result |= (self.apic_page.read::<u32>(OFFSET_ICR32, 0) as u64) << 32;
        };
        result
    }

    fn update_timer_period(&mut self) {
        let init_count: u32 = self.apic_page.read(OFFSET_INIT_COUNT, 0);
        let lvt_timer: u32 = self.apic_page.read(OFFSET_LVT_TIMER, 0);
        if !lvt_masked(lvt_timer) && init_count > 0 {
            let dcr: u32 = self.apic_page.read(OFFSET_DCR, 0);
            // Figure 10-10. Divide Configuration Register
            let dcr_value = (dcr & 0b11) | ((dcr & 0b1000) >> 1);
            let dcr_shift = (dcr_value + 1) & 0b111;
            let interrupt_freq = (self.frequency >> dcr_shift) / init_count as u64;
            let interval = self.ns_to_abs(1_000_000_000u64 / interrupt_freq);
            match lvt_timer_mode(lvt_timer) {
                TIMER_ONE_SHOT => {
                    self.next_timer = Some(mach_abs_time() + interval);
                    self.timer_period = 0;
                }
                TIMER_PERIODIC => {
                    self.next_timer = Some(mach_abs_time() + interval);
                    self.timer_period = interval;
                }
                TIMER_TCS_DDL => unimplemented!("tsc deadline not implemented"),
                _ => unreachable!(),
            }
        } else {
            self.timer_period = 0;
            self.next_timer = None;
        }
    }

    fn mask_all_lvt(&mut self) {
        for offset in &[
            OFFSET_LVT_CMCI,
            OFFSET_LVT_ERROR,
            OFFSET_LVT_LINT0,
            OFFSET_LVT_LINT1,
            OFFSET_LVT_PERF,
            OFFSET_LVT_THERMAL,
            OFFSET_LVT_TIMER,
        ] {
            let v: u32 = self.apic_page.read(*offset, 0);
            self.apic_page.write(v | BIT_LVT_MASKED, *offset, 0);
        }
        self.update_timer_period();
    }

    pub fn fire_external_interrupt(&mut self, vector: u8) {
        self.set_irr(vector);
    }

    pub fn fire_timer_interrupt(&mut self, _vcpu: &VCPU) {
        let timer_lvt: u32 = self.apic_page.read(OFFSET_LVT_TIMER, 0);
        let vector = lvt_vec(timer_lvt);
        debug_assert_eq!(lvt_masked(timer_lvt), false);
        self.set_irr(vector);

        // set up the next timer interrupt
        if self.timer_period == 0 {
            self.next_timer = None;
        } else {
            self.next_timer = Some(mach_abs_time() + self.timer_period);
        }
    }

    // 10.8.3.1 Task and Processor Priorities
    fn update_ppr(&mut self) {
        let tpr: u8 = self.apic_page.read(OFFSET_TPR, 0);
        let isrv = *self.isr_vec.last().unwrap_or(&0);
        let ppr = if priority(tpr) >= priority(isrv) {
            tpr
        } else {
            isrv & 0xf0
        };
        self.apic_page.write(ppr, OFFSET_PPR, 0);
    }

    // 10.8.4 Interrupt Acceptance for Fixed Interrupts
    fn get_vector_from_irr(&self) -> Result<u8, Error> {
        let ppr = self.apic_page.read::<u8>(OFFSET_PPR, 0);
        for i in (0..8).rev() {
            let offset = OFFSET_IRR + i * 0x10;
            let irr: u32 = self.apic_page.read(offset, 0);
            if irr != 0 {
                let vector = i as u8 * 32 + (31 - irr.leading_zeros() as u8);
                if priority(vector) > priority(ppr) {
                    return Ok(vector);
                }
            }
        }
        Err("find no vector".to_string())?
    }

    fn mark_intr_in_service(&mut self, vector: u8) {
        self.clear_irr(vector);
        self.set_isr(vector);
        // vectors in the stack should always increase
        // interrupts with priority 0 is never delivered by APIC, so it is safe
        // to use 0 as the default stack top value.
        debug_assert!(self.isr_vec.last().unwrap_or(&0) < &vector);
        self.isr_vec.push(vector);
        self.update_ppr();
    }

    pub fn inject_interrupt(&mut self, vcpu: &VCPU) -> Result<(), Error> {
        let irr_result = self.get_vector_from_irr();
        if let Ok(vector) = irr_result {
            let rflags = vcpu.read_reg(X86Reg::RFLAGS)?;
            if rflags & FL_IF == FL_IF {
                let blocking = vcpu.read_vmcs(VMCS_GUEST_IGNORE_IRQ)?;
                if blocking & 0b11 == 0 {
                    let entry_info = vcpu.read_vmcs(VMCS_CTRL_VMENTRY_IRQ_INFO)?;
                    if entry_info & (1 << 31) == 0 {
                        let entry_info = (1 << 31) | vector as u64;
                        vcpu.write_vmcs(VMCS_CTRL_VMENTRY_IRQ_INFO, entry_info)?;
                        self.mark_intr_in_service(vector);
                        // error!("injected interrupt {}", vector);
                        return Ok(());
                    }
                }
            }
            let mut ctrl_cpu = vcpu.read_vmcs(VMCS_CTRL_CPU_BASED)?;
            ctrl_cpu |= CPU_BASED_IRQ_WND;
            vcpu.write_vmcs(VMCS_CTRL_CPU_BASED, ctrl_cpu)?;
        }
        Ok(())
    }

    pub fn write(&mut self, offset: usize, mut value: u64) {
        if !(self.x2mode() && offset == OFFSET_ICR0) {
            value &= 0xffffffff;
        }
        match offset {
            OFFSET_ID => {
                if self.x2mode() {
                    // todo: Generate #GP
                    error!("id is read only in x2 mode");
                    unimplemented!()
                } else {
                    if value as u32 != self.apic_page.read(OFFSET_ID, 0) {
                        warn!("ignore changing apic id to {:x}", value);
                    }
                }
            }
            OFFSET_TPR => {
                let current: u8 = self.apic_page.read(OFFSET_TPR, 0);
                let value = value as u8;
                if current != value {
                    info!("OS changed tpr to {:x}", value);
                    self.apic_page.write(value, OFFSET_TPR, 0);
                    self.update_ppr();
                }
            }
            OFFSET_EOI => {
                if let Some(vector) = self.isr_vec.pop() {
                    self.clear_isr(vector);
                    self.update_ppr();
                    if self.tmr_is_set(vector) {
                        // to do: notify io apic
                    }
                } else {
                    error!("meaningless EOI");
                }
            }
            OFFSET_LDR => {
                if self.x2mode() {
                    unimplemented!("write LDR in x2apic")
                } else {
                    info!("OS write {:x} to LDR", value);
                    self.apic_page.write(value as u32, OFFSET_LDR, 0);
                }
            }
            OFFSET_DFR => {
                if self.x2mode() {
                    unimplemented!()
                } else {
                    self.apic_page.write(value as u32, OFFSET_DFR, 0);
                    info!("OS write {:x} to DFR", value);
                }
            }
            OFFSET_SVR => {
                let svr_old: u32 = self.apic_page.read(OFFSET_SVR, 0);
                let enabled_old = svr_old & (1 << 8) > 0;
                let enabled = value & (1 << 8) > 0;
                info!("OS write {:x} to SVR", value);
                if !enabled_old && enabled {
                    // to do: turn on apic
                } else if enabled_old && !enabled {
                    // to do: turn off apic
                    self.mask_all_lvt();
                }
                self.apic_page.write(value as u32, OFFSET_SVR, 0);
            }
            OFFSET_ICR0 => {
                if self.x2mode() {
                    unimplemented!();
                }
                self.apic_page.write(value as u32, OFFSET_ICR0, 0);
                let icr = (self.apic_page.read::<u32>(OFFSET_ICR32, 0) as u64) << 32 | value;
                info!("OS write to ICR0, full icr = {:x}", icr);
            }
            OFFSET_ICR32 => {
                self.apic_page.write(value as u32, OFFSET_ICR32, 0);
            }
            OFFSET_LVT_CMCI => error!("OFFSET_LVT_CMCI"),
            OFFSET_LVT_TIMER => {
                let value = value as u32;
                info!(
                    "new timer: masked: {}, mode: {}, idle: {}, vec: {:x}",
                    lvt_masked(value),
                    lvt_timer_mode(value),
                    lvt_idle(value),
                    lvt_vec(value)
                );
                self.apic_page.write(value, OFFSET_LVT_TIMER, 0);
                self.update_timer_period();
            }
            OFFSET_LVT_THERMAL => {
                error!("OFFSET_LVT_THERMAL, unimplemented");
                self.apic_page.write(value as u32, offset, 0);
            }
            OFFSET_LVT_PERF => {
                error!("OFFSET_LVT_PERF, unimplemented");
                self.apic_page.write(value as u32, offset, 0);
            }
            OFFSET_LVT_LINT0 => {
                error!("OFFSET_LVT_LINT0, {:x}, unimplemented", value);
                self.apic_page.write(value as u32, offset, 0);
            }
            OFFSET_LVT_LINT1 => {
                error!("OFFSET_LVT_LINT1, {:x}, unimplemented", value);
                self.apic_page.write(value as u32, offset, 0);
            }
            OFFSET_LVT_ERROR => {
                error!("OFFSET_LVT_ERROR, {:x}, unimplemented", value);
                self.apic_page.write(value as u32, offset, 0);
            }
            OFFSET_INIT_COUNT => {
                self.apic_page.write(value as u32, OFFSET_INIT_COUNT, 0);
                self.update_timer_period();
            }
            OFFSET_DCR => {
                info!(
                    "guest sets divide configuration register to 0b{:b}",
                    value & 0b1011
                );
                self.apic_page.write(value as u32, OFFSET_DCR, 0);
                self.update_timer_period();
            }
            OFFSET_ESR => {
                self.apic_page.write(self.pending_err, OFFSET_ESR, 0);
                self.pending_err = 0;
            }
            OFFSET_SELF_IPI => unimplemented!("OFFSET_SELF_IPI"),
            _ => error!(
                "guest writes 0x{:x} to a non-existing register 0x{:x}",
                value, offset
            ),
        };
    }

    //10.4.7.1 Local APIC State After Power-Up or Reset
    pub fn reset(&mut self) {
        self.apic_page.write((self.id as u32) << 24, OFFSET_ID, 0);
        self.apic_page
            .write(APIC_VER | (6 << 16) as u32, OFFSET_VER, 0);
        // Figure 10-18. Task-Priority Register (TPR)
        self.apic_page.write(0u32, OFFSET_TPR, 0);
        // Figure 10-15. Arbitration Priority Register (APR)
        self.apic_page.write(0u32, OFFSET_APR, 0);
        // Figure 10-19. Processor-Priority Register (PPR)
        self.apic_page.write(0u32, OFFSET_PPR, 0);
        // Figure 10-13. Logical Destination Register (LDR)
        self.apic_page.write(0u32, OFFSET_LDR, 0);
        // Figure 10-14. Destination Format Register (DFR)
        self.apic_page.write(0xffffffffu32, OFFSET_DFR, 0);
        // Figure 10-23. Spurious-Interrupt Vector Register (SVR)
        self.apic_page.write(0xffu32, OFFSET_SVR, 0);

        // reset IRR, ISR, TMR, Figure 10-20. IRR, ISR and TMR Registers
        for i in 0..(256 / 32) {
            self.apic_page.write(0u32, OFFSET_TMR + i * 0x10, 0);
            self.apic_page.write(0u32, OFFSET_ISR + i * 0x10, 0);
            self.apic_page.write(0u32, OFFSET_ISR + i * 0x10, 0);
        }

        // Figure 10-9. Error Status Register (ESR)
        self.apic_page.write(0u32, OFFSET_ESR, 0);

        // reset LVT, Figure 10-8. Local Vector Table (LVT)
        let lvt_init: u32 = 0x10000;
        self.apic_page.write(lvt_init, OFFSET_LVT_CMCI, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_TIMER, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_THERMAL, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_PERF, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_LINT0, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_LINT1, 0);
        self.apic_page.write(lvt_init, OFFSET_LVT_ERROR, 0);
        self.update_timer_period();

        // Figure 10-12. Interrupt Command Register (ICR)
        self.apic_page.write(0u32, OFFSET_ICR0, 0);
        self.apic_page.write(0u32, OFFSET_ICR32, 0);

        // Figure 10-10. Divide Configuration Register
        self.apic_page.write(0u32, OFFSET_DCR, 0);
        self.apic_page.write(0u32, OFFSET_CURR_COUNT, 0);
        self.apic_page.write(0u32, OFFSET_INIT_COUNT, 0);
    }
}

pub fn apic_access(
    _vcpu: &VCPU,
    gth: &mut GuestThread,
    gpa: usize,
    reg_val: &mut u64,
    _size: u8,
    store: bool,
) -> Result<(), Error> {
    let offset = gpa & 0xfff;
    if store {
        gth.apic.write(offset, *reg_val);
    } else {
        *reg_val = gth.apic.read(offset);
    }
    Ok(())
}
