/* SPDX-License-Identifier: GPL-2.0-only */

// to do: add a channel such that a local apic can send messages to io apic.

use crate::err::Error;
use crate::hv::interrupt_vcpu;
use crate::{GuestThread, VCPU};
use crossbeam_channel::{Receiver, Sender};
#[allow(unused_imports)]
use log::*;
use std::sync::{Arc, Mutex, RwLock};

const IOAPIC_NUM_PINS: u32 = 24;
const IOAPIC_REG_MAX: u32 = 0x10 + 2 * IOAPIC_NUM_PINS - 1;
const IOAPIC_VERSION: u32 = 0x11;

pub struct IoApic {
    id: u32,
    reg: u32,
    arbid: u32,
    pub value: [u32; 2 * IOAPIC_NUM_PINS as usize],
}

impl IoApic {
    pub fn new() -> Self {
        IoApic {
            id: 0,
            reg: 0,
            arbid: 0,
            value: [0; 2 * IOAPIC_NUM_PINS as usize],
        }
    }

    pub fn dispatch(
        intr_senders: Arc<Mutex<Option<Vec<Sender<u8>>>>>, // one sender for one guest thread
        irq_receiver: Receiver<u32>, // receiver for collecting IRQs from hardware
        ioapic: Arc<RwLock<IoApic>>,
        vcpu_ids: Arc<RwLock<Vec<u32>>>, // the actual Hypervisor vcpu id of each guest thread
    ) {
        for irq in irq_receiver.iter() {
            let ioapic = ioapic.read().unwrap();
            let vcpu_ids = vcpu_ids.read().unwrap();
            let entry = ioapic.value[2 * irq as usize] as u64
                | ((ioapic.value[2 * irq as usize + 1] as u64) << 32);
            let vector = (entry & 0xff) as u8;
            let dest = entry >> 56;
            let senders = intr_senders.lock().unwrap();
            if let Some(ref senders) = *senders {
                if entry & (1 << 11) == 0 {
                    // physical mode
                    let dest = (dest & 0b1111) as usize;
                    senders[dest].send(vector).unwrap();
                    interrupt_vcpu(&vcpu_ids[dest..(dest + 1)]).unwrap();
                } else {
                    // logical destination mode
                    for i in 0..8 {
                        if dest & (1 << i) != 0 {
                            senders[i].send(vector).unwrap();
                            interrupt_vcpu(&vcpu_ids[i..(i + 1)]).unwrap();
                        }
                    }
                }
            } else {
                error!(
                    "io apic gets irq 0x{:x}, but has no ways to send it to guest threads",
                    irq
                );
            }
        }
    }

    pub fn write(&mut self, offset: usize, value: u32) {
        if offset == 0 {
            self.reg = value;
        } else {
            match self.reg {
                0 => self.id = value,
                0x10..=IOAPIC_REG_MAX => {
                    self.value[self.reg as usize - 0x10] = value;
                }
                _ => error!(
                    "guest writes 0x{:x} to an invalid/read-only register 0x{:x}",
                    value, self.reg
                ),
            }
        }
    }

    pub fn read(&self, offset: usize) -> u32 {
        if offset == 0 {
            self.reg
        } else {
            match self.reg {
                0 => self.id,
                1 => (IOAPIC_NUM_PINS << 16) | IOAPIC_VERSION, // 0x170011,
                2 => self.arbid,
                0x10..=IOAPIC_REG_MAX => self.value[self.reg as usize - 0x10],
                _ => {
                    error!(
                        "guest reads from an invalid register 0x{:x}. return 0x{:x}",
                        self.reg,
                        u32::MAX
                    );
                    u32::MAX
                }
            }
        }
    }
}

pub fn ioapic_access(
    _vcpu: &VCPU,
    gth: &mut GuestThread,
    gpa: usize,
    reg_val: &mut u64,
    _size: u8,
    store: bool,
) -> Result<(), Error> {
    let offset = gpa & 0xfffff;
    if offset != 0 && offset != 0x10 {
        error!(
            "Bad register offset: {:x} and has to be 0x0 or 0x10",
            offset
        );
        return Ok(());
    }
    let ioapic = &gth.vm.ioapic;
    if store {
        ioapic.write().unwrap().write(offset, *reg_val as u32);
    } else {
        *reg_val = ioapic.read().unwrap().read(offset) as u64;
    }
    Ok(())
}
