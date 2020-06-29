use crate::{Error, GuestThread};
#[allow(unused_imports)]
use log::*;
use std::sync::{Arc, RwLock};

////////////////////////////////////////////////////////////////////////////////
// const
////////////////////////////////////////////////////////////////////////////////

/*
 * Control registers
 */

/* Magic value ("virt" string) - Read Only */
pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;

/* Virtio device version - Read Only */
pub const VIRTIO_MMIO_VERSION: usize = 0x004;

/* Virtio device ID - Read Only */
pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;

/* Virtio vendor ID - Read Only */
pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;

/* Bitmask of the features supported by the device (host)
 * (32 bits per set) - Read Only */
pub const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;

/* Device (host) features set selector - Write Only */
pub const VIRTIO_MMIO_DEVICE_FEATURES_SEL: usize = 0x014;

/* Bitmask of features activated by the driver (guest)
 * (32 bits per set) - Write Only */
pub const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;

/* Activated features set selector - Write Only */
pub const VIRTIO_MMIO_DRIVER_FEATURES_SEL: usize = 0x024;

/* Guest's memory page size in bytes - Write Only */
#[cfg(feature = "virtio_mmio_legacy")]
pub const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028;

/* Queue selector - Write Only */
pub const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;

/* Maximum size of the currently selected queue - Read Only */
pub const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;

/* Queue size for the currently selected queue - Write Only */
pub const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;

/* Used Ring alignment for the currently selected queue - Write Only */
#[cfg(feature = "virtio_mmio_legacy")]
pub const VIRTIO_MMIO_QUEUE_ALIGN: usize = 0x03c;

/* Guest's PFN for the currently selected queue - Read Write */
#[cfg(feature = "virtio_mmio_legacy")]
pub const VIRTIO_MMIO_QUEUE_PFN: usize = 0x040;

/* Ready bit for the currently selected queue - Read Write */
pub const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;

/* Queue notifier - Write Only */
pub const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;

/* Interrupt status - Read Only */
pub const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;

/* Interrupt acknowledge - Write Only */
pub const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;

/* Device status register - Read Write */
pub const VIRTIO_MMIO_STATUS: usize = 0x070;

/* Selected queue's Descriptor Table address, 64 bits in two halves */
pub const VIRTIO_MMIO_QUEUE_DESC_LOW: usize = 0x080;
pub const VIRTIO_MMIO_QUEUE_DESC_HIGH: usize = 0x084;

/* Selected queue's Available Ring address, 64 bits in two halves */
pub const VIRTIO_MMIO_QUEUE_AVAIL_LOW: usize = 0x090;
pub const VIRTIO_MMIO_QUEUE_AVAIL_HIGH: usize = 0x094;

/* Selected queue's Used Ring address, 64 bits in two halves */
pub const VIRTIO_MMIO_QUEUE_USED_LOW: usize = 0x0a0;
pub const VIRTIO_MMIO_QUEUE_USED_HIGH: usize = 0x0a4;

/* Configuration atomicity value */
pub const VIRTIO_MMIO_CONFIG_GENERATION: usize = 0x0fc;

/* The config space is defined by each driver as
 * the per-driver configuration space - Read Write */
pub const VIRTIO_MMIO_CONFIG: usize = 0x100;

////////////////////////////////////////////////////////////////////////////////
// struct
////////////////////////////////////////////////////////////////////////////////

#[repr(packed)] // not necessary
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(packed)] // not necessary
struct VRingAvail {
    flags: u16,
    idx: u16,
    // ring: [u16], //size not available at compile time
}

#[repr(packed)] // not necessary
struct VRingUsedElem {
    id: u32,
    len: u32,
}

#[repr(packed)] // not necessary
struct VRingUsed {
    flags: u16,
    idx: u16,
    // ring: [VRingUsedElem], //size not available at compile time
}

struct VRing<'a> {
    num: u32,
    desc: &'a [VRingDesc],
    avail: &'a VRingAvail,
    used: &'a VRingUsed,
}

struct VirtioVq {
    name: String,
}

struct VirtioVqDev {
    name: String,
    dev_id: u32,
    dev_feat: u64,
    dri_feat: u64,
    cfg: Vec<u32>,
    cfg_d: Vec<u32>,
    vqs: Vec<VirtioVq>,
}

struct VirtioMmioDev {
    addr: u64,
    dev_feat_sel: u32,
    dri_feat_sel: u32,
    qsel: u32,
    isr: u32,
    poke_guest: fn(u8, u32) -> (),
    status: u8,
    cfg_gen: u32,
    vqdev: VirtioVqDev,
    irq: u64,
    vec: u8,
    dest: u32,
}

////////////////////////////////////////////////////////////////////////////////
// config
////////////////////////////////////////////////////////////////////////////////
pub const VIRTIO_CONFIG_S_ACKNOWLEDGE: u8 = 1;
/* We have found a driver for the device. */
pub const VIRTIO_CONFIG_S_DRIVER: u8 = 2;
/* Driver has used its parts of the config, and is happy */
pub const VIRTIO_CONFIG_S_DRIVER_OK: u8 = 4;
/* Driver has finished configuring features */
pub const VIRTIO_CONFIG_S_FEATURES_OK: u8 = 8;
/* Device entered invalid state, driver must reset it */
pub const VIRTIO_CONFIG_S_NEEDS_RESET: u8 = 0x40;
/* We've given up on this device. */
pub const VIRTIO_CONFIG_S_FAILED: u8 = 0x80;

/* Some virtio feature bits (currently bits 28 through 32) are reserved for the
 * transport being used (eg. virtio_ring), the rest are per-device feature
 * bits. */
pub const VIRTIO_TRANSPORT_F_START: u8 = 28;
pub const VIRTIO_TRANSPORT_F_END: u8 = 33;

/* Do we get callbacks when the ring is completely used, even if we've
 * suppressed them? */
#[cfg(feature = "virtio_mmio_legacy")]
pub const VIRTIO_F_NOTIFY_ON_EMPTY: u8 = 24;

/* Can the device handle any descriptor layout? */
#[cfg(feature = "virtio_mmio_legacy")]
pub const VIRTIO_F_ANY_LAYOUT: u8 = 27;

/* v1.0 compliant. */
pub const VIRTIO_F_VERSION_1: u8 = 32;

////////////////////////////////////////////////////////////////////////////////
// mmio
////////////////////////////////////////////////////////////////////////////////

pub const VIRT_MAGIC: u32 = 0x74726976; /* 'virt' */

pub const VIRT_MMIO_VERSION: u32 = 0x2;

pub const VIRT_MMIO_VENDOR: u32 = 0x52414B41; /* 'AKAR' */

fn virtio_mmio_read(dev: Arc<RwLock<VirtioMmioDev>>, gpa: usize, size: u8) -> u32 {
    let mask: u32 = match size {
        1 => 0xff,
        2 => 0xffff,
        4 => 0xffffffff,
        _ => unreachable!(),
    };
    let dev = dev.read().unwrap();
    let offset = gpa - dev.addr as usize;

    // Return 0 for all registers except the magic number,
    // the mmio version, and the device vendor when either
    // there is no vqs on the vqdev.
    if dev.vqdev.vqs.len() == 0 {
        return match offset {
            VIRTIO_MMIO_MAGIC_VALUE => VIRT_MAGIC,
            VIRTIO_MMIO_VERSION => VIRT_MMIO_VERSION,
            VIRTIO_MMIO_VENDOR_ID => VIRT_MMIO_VENDOR,
            _ => 0,
        } & mask;
    }

    if dev.vqdev.dev_id == 0
        && offset != VIRTIO_MMIO_MAGIC_VALUE
        && offset != VIRTIO_MMIO_VERSION
        && offset != VIRTIO_MMIO_DEVICE_ID
    {
        error!("Attempt to read from a register not MagicValue, Version, or DeviceID on a device whose DeviceID is 0x0");
    }

    // Now we know that the host provided a vqdev. As soon as the driver
    // tries to read the magic number, we know it's considering the device.
    // This is a great time to validate the features the host is providing.
    // The host must provide a valid combination of features, or we crash
    // here until the offered feature combination is made valid.
    if offset == VIRTIO_MMIO_MAGIC_VALUE {
        // validate features
        unimplemented!();
    }

    if offset >= VIRTIO_MMIO_CONFIG {
        let offset = offset - VIRTIO_MMIO_CONFIG;
        if dev.status & VIRTIO_CONFIG_S_DRIVER == 0 {
            error!("Driver attempted to read the device-specific configuration space before setting the DRIVER status bit.");
        }
        if offset + (size as usize) > (dev.vqdev.cfg.len() << 2)
            || offset + (size as usize) < offset
        {}
    }
    unimplemented!()
}

fn virtio_mmio_write(
    _gth: &GuestThread,
    _gpa: usize,
    _reg_val: &mut u64,
    _size: u8,
) -> Result<(), Error> {
    unimplemented!()
}

pub fn virtio_mmio(
    _gth: &GuestThread,
    _gpa: usize,
    _reg_val: &mut u64,
    _size: u8,
    _store: bool,
) -> Result<(), Error> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use std::mem::size_of;

    #[test]
    fn virto_struct_test() {
        assert_eq!(size_of::<VRingDesc>(), 16);
        assert_eq!(size_of::<VRingAvail>(), 4);
    }
}
