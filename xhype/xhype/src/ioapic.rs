use crate::{Error, GuestThread};
#[allow(unused_imports)]
use log::*;

const IOAPIC_NUM_PINS: u32 = 24;

pub struct IoApic {
    id: u32,
    reg: u32,
    arbid: u32,
    value: [u32; 256],
}

impl IoApic {
    pub fn new() -> Self {
        IoApic {
            id: 0,
            reg: 0,
            arbid: 0,
            value: [0; 256],
        }
    }
}

fn ioapic_write(gth: &GuestThread, offset: usize, value: u32) {
    let arc_ioapic = { gth.vm.read().unwrap().ioapic.clone() };
    let mut ioapic = arc_ioapic.write().unwrap();
    if offset == 0 {
        info!("ioapic_write set reg {:x}", value);
        ioapic.reg = value;
    } else {
        unimplemented!();
    }
}

fn ioapic_read(gth: &GuestThread, offset: usize) -> u32 {
    let arc_ioapic = { gth.vm.read().unwrap().ioapic.clone() };
    let ioapic = arc_ioapic.read().unwrap();
    let reg = ioapic.reg;
    if offset == 0 {
        reg
    } else {
        let ret = match reg {
            0 => ioapic.id,
            1 => 0x170011,
            2 => ioapic.arbid,
            _ => {
                if reg < (IOAPIC_NUM_PINS * 2 + 0x10) {
                    ioapic.value[reg as usize]
                } else {
                    warn!("IO APIC read bad reg {:x}", reg);
                    0xffffffff
                }
            }
        };
        info!("IO APIC read reg {:x} return {:x}", reg, ret);
        ret
    }
}

pub fn ioapic_access(
    gth: &GuestThread,
    gpa: usize,
    reg_val: &mut u64,
    _size: u8,
    store: bool,
) -> Result<(), Error> {
    let offset = gpa & 0xfffff;
    if offset != 0 && offset != 0x10 {
        warn!(
            "Bad register offset: {:x} and has to be 0x0 or 0x10",
            offset
        );
        return Ok(());
    }
    if store {
        ioapic_write(gth, offset, *reg_val as u32);
    } else {
        *reg_val = ioapic_read(gth, offset) as u64;
    }
    Ok(())
}
