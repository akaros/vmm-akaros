/* SPDX-License-Identifier: GPL-2.0-only */

//https://www.freebsd.org/doc/en_US.ISO8859-1/articles/serial-uart/index.html

use bitfield::bitfield;
use log::*;
use std::collections::VecDeque;

// offset 0x1, Interrupt Enable Register (IER)
bitfield! {
    #[derive(Copy, Clone, Debug)]
    struct Ier(u8);
    u8;
    edssi, _: 3,3; // Enable Modem Status Interrupt
    elsi, _: 2,2;  // Enable Receiver Line Status Interrupt
    etbei, _: 1,1; // Enable Transmitter Holding Register Empty Interrupt
    erbfi, _: 0,0; // Enable Received Data Available Interrupt
}

impl Default for Ier {
    fn default() -> Self {
        Ier(0) // disable all interrupts as default
    }
}

// offset 0x2, write, FIFO Control Register (FCR)
bitfield! {
    #[derive(Copy, Clone, Debug, Default)]
    struct Fcr(u8);
    u8;
    rtb, set_rtb: 6,7;    // receiver trigger bit
    dms, set_dms: 3,3;    // DMA Mode Select
    tfr, set_tfr: 2,2;    // Transmit FIFO Reset
    rfr, set_rfr: 1,1;    // Receiver FIFO Reset
    fifo, set_fifi: 0,0;  // 16550 FIFO Enable
}

// offset 0x2, read, Interrupt Identification Register
bitfield! {
    #[derive(Copy, Clone, Debug, Default)]
    struct Iir(u8);
    u8;
    intr_id, _: 1,3; // Interrupt ID
    pending, _: 0,0; // Interrupt Pending Bit
}

const DATA_AVAILABLE: u8 = 0b010;
const ROOM_AVAILABLE: u8 = 0b001;

// offset 0x3, Line Control Register (LCR)
bitfield! {
    #[derive(Copy, Clone, Debug)]
    struct Lcr(u8);
    u8;
    dlab, _: 7, 7;
    set_break, _: 6,6;
    stick_parity, _: 5,5;
    eps, _: 4,4;
    pen, _: 3,3;
    stb, _: 2,2;
    word_length, _: 0,1;
}

impl Default for Lcr {
    fn default() -> Self {
        Lcr(0b00000011) // 8 data bits as default
    }
}

// offset 0x4, Modem Control Register
bitfield! {
    #[derive(Copy, Clone, Debug)]
    struct Mcr(u8);
    u8;
    rts, _: 1,1;
    dtr, _: 0,0; // Data Terminal Ready
}

impl Default for Mcr {
    fn default() -> Self {
        Mcr(0) // Data Terminal Ready
    }
}

// offset 0x5, Line Status Register (LSR)
bitfield! {
    #[derive(Copy, Clone, Debug)]
    struct Lsr(u8);
    u8;
    err_fifo, _: 7,7;
    temt, _: 6,6; // transmitter empty
    thre, _: 5,5; // transmitter holding register empty
    bi, _: 4,4; // break interrupt
    fe, _: 3,3; // framing error
    pe, _: 2,2; // parity error
    oe, _: 1,1; // overrun error
    ready, _: 0,0; // data ready
}

impl Default for Lsr {
    fn default() -> Self {
        Lsr(0b00100000) // Transmitter Holding Register Empty (THRE)
    }
}

// TO-DO: send interrupts

#[derive(Debug, Default)]
pub struct Serial {
    ier: Ier, // 0x1, Interrupt Enable Register (IER)
    fcr: Fcr, // 0x2, write, FIFO Control Register (FCR)
    iir: Iir, // 0x2, read, Interrupt Identification Register
    lcr: Lcr, // 0x3, Line Control Register (LCR)
    mcr: Mcr, // 0x4, Modem Control Register (MCR)
    lsr: Lsr, // 0x5, Line Status Register (LSR)
    msr: u8,  // 0x6, Modem Status Register (MSR)
    scr: u8,  // 0x7, Scratch Register (SCR)
    divisor: u16,
    in_data: VecDeque<u8>,
}

impl Serial {
    pub fn read(&mut self, offset: u16) -> u8 {
        let result = match offset {
            0 => {
                if self.lcr.dlab() == 0 {
                    self.in_data.pop_front().unwrap_or({
                        warn!("OS reads from serial port. no available bytes, return 0xff");
                        0xff
                    })
                } else {
                    (self.divisor & 0xff) as u8
                }
            }
            1 => {
                if self.lcr.dlab() == 0 {
                    self.ier.0
                } else {
                    (self.divisor >> 8) as u8
                }
            }
            2 => {
                if self.in_data.len() > 0 {
                    DATA_AVAILABLE << 1
                } else {
                    ROOM_AVAILABLE << 1
                }
            }
            3 => self.lcr.0,
            4 => self.mcr.0,
            5 => self.lsr.0,
            6 => self.msr,
            7 => self.scr,
            _ => unreachable!("offset {}", offset),
        };
        info!("read {:08b} from offset {}", result, offset);
        result
    }

    pub fn write(&mut self, offset: u16, value: u8) {
        info!("write {:08b} to offset {}", value, offset);
        match offset {
            0 => {
                if self.lcr.dlab() == 0 {
                    print!("{}", value as char)
                } else {
                    self.divisor &= !0xff;
                    self.divisor |= value as u16;
                }
            }
            1 => {
                if self.lcr.dlab() == 0 {
                    self.ier.0 = value
                } else {
                    self.divisor &= 0xff;
                    self.divisor |= (value as u16) << 8;
                }
            }
            2 => self.fcr.0 = value,
            3 => self.lcr.0 = value,
            4 => self.mcr.0 = value,
            5 => self.lsr.0 = value,
            6 => self.msr = value,
            7 => self.scr = value,
            _ => unreachable!("offset {}, value = {:b}", offset, value),
        }
    }
}
