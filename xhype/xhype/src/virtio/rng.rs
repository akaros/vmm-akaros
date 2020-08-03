/* SPDX-License-Identifier: GPL-2.0-only */

use super::consts::*;
use super::virtq::*;
use super::{AddressConverter, Sender, VirtioDevCfg, VirtioDevice, VirtioId};
#[allow(unused_imports)]
use log::*;
use rand::random;
use std::slice;
use std::sync::{Arc, RwLock};

struct RngDescHandler {}

impl VirtqDescHandle for RngDescHandler {
    fn handle_desc_chain(
        &mut self,
        virtq: &Virtq<usize>,
        index: u16,
        gpa2hva: &AddressConverter,
    ) -> u32 {
        let (desc_chain, writable_len) = virtq.get_desc_chain(index, |gpa| gpa2hva(gpa));
        debug_assert_eq!(desc_chain.len(), writable_len);
        let mut total_size = 0;
        for (addr, len) in desc_chain.into_iter() {
            let buf = unsafe { slice::from_raw_parts_mut(addr as *mut u8, len) };
            for byte in buf.iter_mut() {
                *byte = random();
            }
            trace!("virtio-rng writes random bytes to guest: {:02x?}", buf);
            total_size += len;
        }
        total_size as u32
    }
}

pub struct VirtioRngCfg {
    gen: u32,
}

impl VirtioDevCfg for VirtioRngCfg {
    fn write(&mut self, _offset: usize, _size: u8, _value: u32) -> Option<()> {
        None
    }

    fn read(&self, _offset: usize, _size: u8) -> Option<u32> {
        None
    }

    fn reset(&mut self) {
        self.gen += 1;
    }

    fn generation(&self) -> u32 {
        self.gen
    }
}

impl VirtioDevice {
    pub fn new_rng(
        name: String,
        irq: u32,
        irq_sender: Sender<u32>,
        gpa2hva: AddressConverter,
    ) -> Self {
        let rng_cfg = VirtioRngCfg { gen: 0 };
        let isr = Arc::new(RwLock::new(0));
        let handler = RngDescHandler {};
        let req_q = VirtqManager::new(
            format!("{}_req", name),
            64,
            irq,
            irq_sender,
            isr.clone(),
            gpa2hva.clone(),
            handler,
        );
        let vqs = vec![req_q];
        VirtioDevice {
            name,
            dev_id: VirtioId::Entropy,
            dev_feat: 1 << VIRTIO_F_VERSION_1,
            dri_feat: 0,
            dev_feat_sel: 0,
            dri_feat_sel: 0,
            qsel: 0,
            vqs,
            cfg: Box::new(rng_cfg),
            isr,
            status: 0,
            cfg_gen: 0,
            irq,
        }
    }
}
