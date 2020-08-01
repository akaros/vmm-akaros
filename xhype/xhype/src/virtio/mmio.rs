/* SPDX-License-Identifier: GPL-2.0-only */

use super::consts::*;
use super::virtq::*;
use super::VirtioDevice;
use crate::{err::Error, GuestThread, VCPU};
#[allow(unused_imports)]
use log::*;

// The implementation is based on https://github.com/akaros/akaros/blob/master/user/vmm/virtio_mmio.c

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

pub const VIRT_MAGIC: u32 = 0x74726976; /* 'virt' */

pub const VIRT_MMIO_VERSION: u32 = 0x2;

pub const VIRT_MMIO_VENDOR: u32 = 0x52414B41; /* 'AKAR' */

pub struct VirtioMmioDev {
    pub(crate) addr: usize,
    pub(crate) dev: VirtioDevice,
}

impl VirtioMmioDev {
    pub fn reset(&mut self) {
        self.dev.dri_feat = 0;
        self.dev.status = 0;
        *self.dev.isr.write().unwrap() = 0;
        for vq in self.dev.vqs.iter_mut() {
            vq.qready = 0;
        }
        self.dev.cfg.reset();
    }

    pub fn selected_vq(&mut self) -> Option<&mut VirtqManager> {
        if (self.dev.qsel as usize) < self.dev.vqs.len() {
            let vq = &mut self.dev.vqs[self.dev.qsel as usize];
            if vq.qready != 0 {
                error!(
                    "guest accesses virtq {}, which has nonzero QueueReady.",
                    vq.name
                );
                None
            } else {
                Some(vq)
            }
        } else {
            error!(
                "guest accesses dev {}'s virtq with an invalid qsel {}",
                self.dev.name, self.dev.qsel
            );
            None
        }
    }
}

fn virtio_mmio_read(mmio_dev: &VirtioMmioDev, gpa: usize, size: u8) -> u32 {
    let offset = gpa - mmio_dev.addr as usize;

    if offset >= VIRTIO_MMIO_CONFIG {
        let offset = offset - VIRTIO_MMIO_CONFIG;
        let ret = if let Some(val) = mmio_dev.dev.cfg.read(offset, size) {
            val
        } else {
            error!(
                "the driver reads config of device {} with invalid size or offset: ({}, 0x{:x})",
                mmio_dev.dev.name, size, offset
            );
            0
        };
        return ret;
    }

    if size != 4 || offset % 4 != 0 {
        error!(
            "The driver must only use 32 bit wide and aligned reads for \
        reading the control registers on the MMIO transport. See \
        virtio-v1.0-cs04 4.2.2.2 MMIO Device Register Layout."
        );
    }

    match offset {
        VIRTIO_MMIO_MAGIC_VALUE => VIRT_MAGIC,

        VIRTIO_MMIO_VERSION => VIRT_MMIO_VERSION,

        VIRTIO_MMIO_DEVICE_ID => mmio_dev.dev.dev_id as u32,

        // Virtio Subsystem Vendor ID
        VIRTIO_MMIO_VENDOR_ID => VIRT_MMIO_VENDOR,

        // Flags representing features the device supports
        VIRTIO_MMIO_DEVICE_FEATURES => {
            if mmio_dev.dev.status & VIRTIO_CONFIG_S_DRIVER == 0 {
                error!(
                    "Attempt to read device features before setting the \
                DRIVER status bit. See virtio-v1.0-cs04 s3.1.1 Device Initialization"
                );
            }
            (if mmio_dev.dev.dev_feat_sel > 0 {
                mmio_dev.dev.dev_feat >> 32 // high 32 bits requested
            } else {
                mmio_dev.dev.dev_feat & 0xffffffff // low 32 bits requested
            }) as u32
        }

        VIRTIO_MMIO_QUEUE_NUM_MAX => {
            if mmio_dev.dev.qsel as usize >= mmio_dev.dev.vqs.len() {
                0
            } else {
                mmio_dev.dev.vqs[mmio_dev.dev.qsel as usize].qnum_max
            }
        }

        VIRTIO_MMIO_QUEUE_READY => {
            if mmio_dev.dev.qsel as usize >= mmio_dev.dev.vqs.len() {
                0
            } else {
                mmio_dev.dev.vqs[mmio_dev.dev.qsel as usize].qready
            }
        }

        VIRTIO_MMIO_INTERRUPT_STATUS => *mmio_dev.dev.isr.read().unwrap(),

        VIRTIO_MMIO_STATUS => mmio_dev.dev.status as u32,

        VIRTIO_MMIO_CONFIG_GENERATION => mmio_dev.dev.cfg_gen,

        _ => {
            error!(
                "driver attempts to read write-only or invalid register offset {:x} of device {}",
                offset, mmio_dev.dev.name
            );
            0
        }
    }
}

fn virtio_mmio_write(mmio_dev: &mut VirtioMmioDev, gpa: usize, size: u8, value: u32) {
    let offset = gpa - mmio_dev.addr as usize;

    if offset >= VIRTIO_MMIO_CONFIG {
        let offset = offset - VIRTIO_MMIO_CONFIG;
        if mmio_dev.dev.cfg.write(offset, size, value).is_none() {
            error!(
                "the driver writes config of device {} with invalid size or offset: ({}, 0x{:x})",
                mmio_dev.dev.name, size, offset
            );
        }
        return;
    }

    if size != 4 || offset % 4 != 0 {
        error!(
            "The driver must only use 32 bit wide and aligned reads for \
        reading the control registers on the MMIO transport. See \
        virtio-v1.0-cs04 4.2.2.2 MMIO Device Register Layout."
        );
    }

    match offset {
        VIRTIO_MMIO_DEVICE_FEATURES_SEL => mmio_dev.dev.dev_feat_sel = value,

        VIRTIO_MMIO_DRIVER_FEATURES => {
            if mmio_dev.dev.status & VIRTIO_CONFIG_S_FEATURES_OK > 0 {
                error!(
                    "The driver is not allowed to activate new features after \
                setting FEATURES_OK"
                );
            } else if mmio_dev.dev.dri_feat_sel > 0 {
                mmio_dev.dev.dri_feat &= 0xffffffff;
                mmio_dev.dev.dri_feat |= (value as u64) << 32;
            } else {
                mmio_dev.dev.dri_feat &= 0xffffffffu64 << 32;
                mmio_dev.dev.dri_feat |= value as u64;
            }
        }

        VIRTIO_MMIO_DRIVER_FEATURES_SEL => mmio_dev.dev.dri_feat_sel = value,

        VIRTIO_MMIO_QUEUE_SEL => mmio_dev.dev.qsel = value,

        VIRTIO_MMIO_QUEUE_NUM => {
            let qsel = mmio_dev.dev.qsel as usize;
            if qsel < mmio_dev.dev.vqs.len() {
                let vq = &mut mmio_dev.dev.vqs[qsel];
                if value <= vq.qnum_max {
                    vq.virtq.num = value;
                } else {
                    error!(
                        "write a value to QueueNum which is greater than \
                    QueueNumMax"
                    );
                }
            } else {
                error!("qsel has an invalid value. qsel >= vqs.len()");
            }
        }

        VIRTIO_MMIO_QUEUE_READY => {
            let qsel = mmio_dev.dev.qsel as usize;
            if qsel < mmio_dev.dev.vqs.len() {
                let vq = &mut mmio_dev.dev.vqs[qsel];
                if vq.qready == 0x0 && value == 0x1 {
                    vq.virtq_sender.send(vq.virtq.clone()).unwrap();
                } else if vq.qready == 0x1 && value == 0x0 {
                    // send a index None to indicate that this virtq is not
                    // available any more
                    vq.task_sender.send(None).unwrap();
                }
                vq.qready = value;
            } else {
                error!("qsel has an invalid value. qsel >= vqs.len()");
            }
        }

        VIRTIO_MMIO_QUEUE_NOTIFY => {
            let q_index = value as usize;
            if mmio_dev.dev.status & VIRTIO_CONFIG_S_DRIVER_OK == 0 {
                error!(
                    "{} notify device before DRIVER_OK is set",
                    mmio_dev.dev.name
                );
            } else if q_index < mmio_dev.dev.vqs.len() {
                let vq = &mmio_dev.dev.vqs[q_index];
                vq.task_sender.send(Some(())).unwrap();
            }
        }

        VIRTIO_MMIO_INTERRUPT_ACK => {
            if value & !0x3 > 0 {
                error!(
                    "{} set undefined bits in InterruptAck register, value = 0x{:x}",
                    mmio_dev.dev.name, value
                );
            }
            *mmio_dev.dev.isr.write().unwrap() &= !value;
        }

        VIRTIO_MMIO_STATUS => {
            let mut value = value as u8;
            if value == 0 {
                mmio_dev.reset();
            } else if mmio_dev.dev.status & !value != 0 {
                error!("The driver must not clear any device status bits, except as a result of resetting the device.")
            } else if mmio_dev.dev.status & VIRTIO_CONFIG_S_FAILED != 0 && mmio_dev.dev.status != value {
                error!("The driver must reset the device after setting the FAILED status bit, before attempting to re-initialize the device.");
            } else {
                let dev = &mmio_dev.dev;
                if value & VIRTIO_CONFIG_S_ACKNOWLEDGE > 0 {
                    if value & VIRTIO_CONFIG_S_DRIVER > 0 {
                        if value & VIRTIO_CONFIG_S_FEATURES_OK > 0 {
                            if mmio_dev.dev.status & VIRTIO_CONFIG_S_FEATURES_OK > 0 {
                                if value & VIRTIO_CONFIG_S_DRIVER_OK > 0 {
                                    info!("the device {} is alive", mmio_dev.dev.name);
                                } else {
                                    warn!("feature bits are verified but driver is not ok");
                                }
                            } else {
                                if let Err(s) = mmio_dev.dev.verify_feat() {
                                    error!("feature bits verification failed: {}, dev: {}", s, dev.name);
                                    value &= !VIRTIO_CONFIG_S_FEATURES_OK;
                                } else {
                                    info!("feature bits verification succeeded. dev: {}", dev.name);
                                }
                                if value & VIRTIO_CONFIG_S_DRIVER_OK != 0 {
                                    error!("the driver cannot set feature_ok and driver_ok at the same time");
                                    value &= !VIRTIO_CONFIG_S_DRIVER_OK;
                                } else {
                                    info!("the driver will re-verify feature-ok bit of dev {} is set.", dev.name);
                                }
                            }
                        } else {
                            info!("the driver is supposed to read {}'s feature bits later", mmio_dev.dev.name);
                        }
                    } else {
                        info!(
                            "the driver does not know how to drive {} for now",
                            mmio_dev.dev.name
                        );
                    }
                } else {
                    warn!(
                        "The driver ignored the device, {}",
                        mmio_dev.dev.name
                    );
                }
            }
            mmio_dev.dev.status = value;
            info!("dev {} status = {:b}", mmio_dev.dev.name, value);
        }

        VIRTIO_MMIO_QUEUE_DESC_LOW => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_low(&mut vq.virtq.desc, value);
            }
        }

        VIRTIO_MMIO_QUEUE_DESC_HIGH => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_high(&mut vq.virtq.desc, value);
            }
        }

        VIRTIO_MMIO_QUEUE_AVAIL_LOW => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_low(&mut vq.virtq.avail, value);
            }
        }

        VIRTIO_MMIO_QUEUE_AVAIL_HIGH => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_high(&mut vq.virtq.avail, value);
            }
        }

        VIRTIO_MMIO_QUEUE_USED_LOW => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_low(&mut vq.virtq.used, value);
            }
        }

        VIRTIO_MMIO_QUEUE_USED_HIGH => {
            if let Some(vq) = mmio_dev.selected_vq() {
                set_u64_high(&mut vq.virtq.used, value);
            }
        }

        _ => error!(
            "driver attempts to write 0x{:x} to read-only or invalid register offset {:x} of device {}",
            value, offset, mmio_dev.dev.name
        ),
    }
}

fn set_u64_low(num: &mut u64, value: u32) {
    *num &= !0xffffffff;
    *num |= value as u64;
}

fn set_u64_high(num: &mut u64, value: u32) {
    *num &= 0xffffffff;
    *num |= (value as u64) << 32;
}

pub fn virtio_mmio(
    _vcpu: &VCPU,
    gth: &mut GuestThread,
    gpa: usize,
    reg_val: &mut u64,
    size: u8,
    store: bool,
) -> Result<(), Error> {
    let mask = match size {
        1 => 0xff,
        2 => 0xffff,
        4 => 0xffffffff,
        _ => unreachable!(),
    };
    let dev_index = (gpa - gth.vm.virtio_base) >> 12;
    if store {
        let mut dev = gth.vm.virtio_mmio_devices[dev_index].lock().unwrap();
        debug!(
            "virtio-mmio: store 0x{:x} to gpa = 0x{:x}, size = {}",
            *reg_val & mask,
            gpa,
            size
        );
        virtio_mmio_write(&mut *dev, gpa, size, *reg_val as u32)
    } else {
        let dev = &gth.vm.virtio_mmio_devices[dev_index].lock().unwrap();
        let val = virtio_mmio_read(dev, gpa, size);
        debug!(
            "virtio-mmio: read from gpa = 0x{:x}, size = {}, return {:x}",
            gpa, size, val
        );
        *reg_val = val as u64;
    }
    Ok(())
}
