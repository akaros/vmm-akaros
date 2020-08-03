/* SPDX-License-Identifier: GPL-2.0-only */

pub mod mmio;
pub mod net;
pub mod rng;
pub mod virtq;

use crate::AddressConverter;
use crossbeam_channel::unbounded as channel;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, RwLock};
use virtq::*;

type TaskSender = Sender<Option<()>>;
type TaskReceiver = Receiver<Option<()>>;
type VirtqSender = Sender<Virtq<u64>>;
type VirtqReceiver = Receiver<Virtq<u64>>;
type IrqSender = Sender<u32>;

pub mod consts {
    /* We have seen device and processed generic fields (VIRTIO_CONFIG_F_VIRTIO) */
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
    /* v1.0 compliant. */
    pub const VIRTIO_F_VERSION_1: u8 = 32;

    pub const VIRTIO_INT_VRING: u32 = 1 << 0;
    pub const VIRTIO_INT_CONFIG: u32 = 1 << 1;
}

// virtio-v1.0-cs04 s4 Device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioId {
    Reserved = 0,
    Net = 1,
    Block = 2,
    Console = 3,
    Entropy = 4,
    BalloonTraditional = 5,
    IoMemory = 6,
    RpMsg = 7,
    ScsiHost = 8,
    Transport9P = 9, // 9P transport
    Mac80211Wlan = 10,
    RProcSerial = 11,
    Caif = 12,
    Balloon = 13,
    GPU = 16,
    Timer = 17,
    Input = 18,
}

pub trait VirtioDevCfg {
    fn reset(&mut self);
    fn generation(&self) -> u32;
    fn read(&self, offset: usize, size: u8) -> Option<u32>;
    fn write(&mut self, offset: usize, size: u8, value: u32) -> Option<()>;
}

pub struct VirtioDevice {
    name: String,
    dev_id: VirtioId,
    dev_feat: u64,
    dri_feat: u64,
    dev_feat_sel: u32,
    dri_feat_sel: u32,
    qsel: u32,
    isr: Arc<RwLock<u32>>, // interrupt status
    status: u8,
    cfg_gen: u32,
    cfg: Box<dyn VirtioDevCfg + Send + Sync + 'static>,
    vqs: Vec<VirtqManager>,
    pub(crate) irq: u32,
}

impl VirtioDevice {
    pub fn verify_feat(&self) -> Result<(), &'static str> {
        if self.dri_feat & (1 << consts::VIRTIO_F_VERSION_1) == 0 {
            return Err("A driver must accept the VIRTIO_F_VERSION_1 feature bit");
        }
        if self.dri_feat & !self.dev_feat != 0 {
            return Err("driver activated features that are not supported by the device");
        }
        Ok(())
    }
}
