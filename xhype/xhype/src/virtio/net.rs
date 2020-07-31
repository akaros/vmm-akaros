/* SPDX-License-Identifier: GPL-2.0-only */

use super::consts::*;
use super::mmio::{virtq_server, VirtioMmioDev};
use super::virtq::*;
use super::{AddressConverter, Sender, VirtioDevCfg, VirtioDevice, VirtioId};
#[allow(unused_imports)]
use log::*;
use std::io::IoSliceMut;
use std::slice;
use std::sync::{Arc, RwLock};

pub const VIRTIO_HEADER_SIZE: usize = 12;

pub const VIRTIO_NET_F_CSUM: u64 = 0; /* Host handles pkts w/ partial csum */
pub const VIRTIO_NET_F_GUEST_CSUM: u64 = 1; /* Guest handles pkts w/ partial csum */
pub const VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: u64 = 2; /* Dynamic offload configuration. */
pub const VIRTIO_NET_F_MTU: u64 = 3;
pub const VIRTIO_NET_F_MAC: u64 = 5; /* Host has given MAC address. */
pub const VIRTIO_NET_F_GUEST_TSO4: u64 = 7; /* Guest can handle TSOv4 in. */
pub const VIRTIO_NET_F_GUEST_TSO6: u64 = 8; /* Guest can handle TSOv6 in. */
pub const VIRTIO_NET_F_GUEST_ECN: u64 = 9; /* Guest can handle TSO[6] w/ ECN in. */
pub const VIRTIO_NET_F_GUEST_UFO: u64 = 10; /* Guest can handle UFO in. */
pub const VIRTIO_NET_F_HOST_TSO4: u64 = 11; /* Host can handle TSOv4 in. */
pub const VIRTIO_NET_F_HOST_TSO6: u64 = 12; /* Host can handle TSOv6 in. */
pub const VIRTIO_NET_F_HOST_ECN: u64 = 13; /* Host can handle TSO[6] w/ ECN in. */
pub const VIRTIO_NET_F_HOST_UFO: u64 = 14; /* Host can handle UFO in. */
pub const VIRTIO_NET_F_MRG_RXBUF: u64 = 15; /* Host can merge receive buffers. */
pub const VIRTIO_NET_F_STATUS: u64 = 16; /* virtio_net_config.status available */
pub const VIRTIO_NET_F_CTRL_VQ: u64 = 17; /* Control channel available */
pub const VIRTIO_NET_F_CTRL_RX: u64 = 18; /* Control channel RX mode support */
pub const VIRTIO_NET_F_CTRL_VLAN: u64 = 19; /* Control channel VLAN filtering */
pub const VIRTIO_NET_F_CTRL_RX_EXTRA: u64 = 20; /* Extra RX mode control support */
pub const VIRTIO_NET_F_GUEST_ANNOUNCE: u64 = 21; /* Guest can announce device on the network */

pub const VMNET_SUCCESS: u32 = 1000;

#[repr(C)]
struct VmPktDesc<'a> {
    vm_pkt_size: usize,
    vm_pkt_iov: *mut IoSliceMut<'a>,
    vm_pkt_iovcnt: u32,
    vm_flags: u32,
}

extern "C" {
    fn vmnet_read_blocking(interface: usize, packets: *mut VmPktDesc, pktcnt: *mut u32) -> u32;
    fn vmnet_write(interface: usize, packets: *const VmPktDesc, pktcnt: *mut u32) -> u32;
    fn create_interface(interface: *mut usize, mac: *mut u8, mtu: *mut u16) -> u32;
}

fn net_rx_handler(
    virtq: &Virtq<usize>,
    index: u16,
    gpa2hva: &AddressConverter,
    interface: usize,
) -> u32 {
    let (readable, writable) = virtq.get_avail(index, |gpa| gpa2hva(gpa));
    debug_assert_eq!(readable.len(), 0);
    drop(readable);
    let mut trim_length = VIRTIO_HEADER_SIZE;
    let mut iov = Vec::with_capacity(writable.len());
    let mut total_size = 0;
    for (mut addr, mut len) in writable.iter() {
        if trim_length > 0 {
            if len > trim_length {
                len -= trim_length;
                addr += trim_length;
                trim_length = 0;
            } else {
                trim_length -= len;
                continue;
            }
        }
        total_size += len;
        let io_slice = unsafe { slice::from_raw_parts_mut(addr as *mut u8, len) };
        iov.push(IoSliceMut::new(io_slice));
    }
    let mut vmpktdesc = VmPktDesc {
        vm_flags: 0,
        vm_pkt_iovcnt: iov.len() as u32,
        vm_pkt_size: total_size,
        vm_pkt_iov: iov.as_mut_ptr(),
    };
    let mut pkt_count = 1;
    let ret = unsafe { vmnet_read_blocking(interface, &mut vmpktdesc, &mut pkt_count) };
    if ret == VMNET_SUCCESS && pkt_count == 1 {
        info!("net_rx_srv, get {} bytes", vmpktdesc.vm_pkt_size);
        let (addr, _) = writable[0];
        let first_buffer = unsafe { slice::from_raw_parts_mut(addr as *mut u16, 8) };
        for i in 0..5 {
            first_buffer[i] = 0;
        }
        first_buffer[5] = 1;
        let content = format!("{:04x?}", first_buffer);
        error!("rx header {}", content);
        (vmpktdesc.vm_pkt_size + VIRTIO_HEADER_SIZE) as u32
    } else {
        error!("vmnet_read() returns {}", ret);
        0
    }
}

fn net_rx_srv(iref: usize) -> VirtioVqSrv {
    Box::new(move |task_rx, virtq_rx, irq, irq_tx, isr, gpa2hva| {
        virtq_server(
            task_rx,
            virtq_rx,
            irq,
            irq_tx,
            isr,
            &gpa2hva,
            |virtq: &Virtq<usize>, index: u16, converter| {
                net_rx_handler(virtq, index, converter, iref)
            },
        );
    })
}

fn net_tx_handler(
    virtq: &Virtq<usize>,
    index: u16,
    gpa2hva: &AddressConverter,
    interface: usize,
) -> u32 {
    let (readable, writable) = virtq.get_avail(index, |gpa| gpa2hva(gpa));
    debug_assert_eq!(writable.len(), 0);
    drop(writable);
    let mut trim_length = VIRTIO_HEADER_SIZE;
    let mut iov = Vec::with_capacity(readable.len());
    let mut total_size = 0;
    for (mut addr, mut len) in readable.into_iter() {
        if trim_length > 0 {
            if len > trim_length {
                len -= trim_length;
                addr += trim_length;
                trim_length = 0;
            } else {
                trim_length -= len;
                continue;
            }
        }
        total_size += len;
        let io_slice = unsafe { slice::from_raw_parts_mut(addr as *mut u8, len) };

        iov.push(IoSliceMut::new(io_slice));
    }
    let vmpktdesc = VmPktDesc {
        vm_flags: 0,
        vm_pkt_iovcnt: iov.len() as u32,
        vm_pkt_size: total_size,
        vm_pkt_iov: iov.as_mut_ptr(),
    };
    let mut pkt_count = 1;
    let ret = unsafe { vmnet_write(interface, &vmpktdesc, &mut pkt_count) };
    if ret == VMNET_SUCCESS && pkt_count == 1 {
        info!("net_tx_srv, write {} bytes", vmpktdesc.vm_pkt_size);
    } else {
        error!("vmnet_write returns {}", ret);
    }
    0
}

fn net_tx_srv(interface: usize) -> VirtioVqSrv {
    Box::new(move |task_rx, virtq_rx, irq, irq_tx, isr, gpa2hva| {
        virtq_server(
            task_rx,
            virtq_rx,
            irq,
            irq_tx,
            isr,
            &gpa2hva,
            |virtq: &Virtq<usize>, index: u16, converter| {
                net_tx_handler(virtq, index, converter, interface)
            },
        );
    })
}

pub struct VirtioNetCfg {
    pub mac: [u8; 6],
    pub status: u16,
    pub max_virtqueue_pairs: u16,
    pub mtu: u16,
    pub gen: u32,
}

impl VirtioDevCfg for VirtioNetCfg {
    fn write(&mut self, _offset: usize, _size: u8, _value: u32) -> Option<()> {
        // virtio-v1.1-csprd01, 5.1.4
        None
    }

    fn read(&self, offset: usize, size: u8) -> Option<u32> {
        match (size, offset) {
            (1, 0..=6) => Some(self.mac[offset] as u32),
            (2, 6) => Some(self.status as u32),
            (2, 8) => Some(self.max_virtqueue_pairs as u32),
            (2, 10) => Some(self.mtu as u32),
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.status = 0;
        self.gen += 1;
    }

    fn generation(&self) -> u32 {
        self.gen
    }
}

impl VirtioDevice {
    pub fn new_vmnet(
        name: String,
        irq: u32,
        irq_sender: Sender<u32>,
        gpa2hva: AddressConverter,
    ) -> Self {
        let mut interface = 0;
        let mut mac = vec![0u8; 17];
        let mut mtu = 0u16;
        let ret = unsafe { create_interface(&mut interface, mac.as_mut_ptr(), &mut mtu) };
        if ret != 0 {
            panic!("cannot create vmnet interface. root privilege is required.");
        }
        let mac_vec: Vec<u8> = String::from_utf8(mac)
            .unwrap()
            .split(':')
            .map(|c| u8::from_str_radix(c, 16).unwrap())
            .collect();
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&mac_vec);
        let net_cfg = VirtioNetCfg {
            mac,
            status: 0,
            max_virtqueue_pairs: 1,
            mtu,
            gen: 0,
        };

        let isr = Arc::new(RwLock::new(0));
        let rx = VirtqManager::new(
            format!("{}_rx", name),
            64,
            net_rx_srv(interface),
            irq,
            irq_sender.clone(),
            isr.clone(),
            gpa2hva.clone(),
        );
        let tx = VirtqManager::new(
            format!("{}_tx", name),
            64,
            net_tx_srv(interface),
            irq,
            irq_sender.clone(),
            isr.clone(),
            gpa2hva.clone(),
        );
        let vqs = vec![rx, tx];

        VirtioDevice {
            name,
            dev_id: VirtioId::Net,
            dev_feat: (1 << VIRTIO_F_VERSION_1) | (1 << VIRTIO_NET_F_MAC) | (1 << VIRTIO_NET_F_MTU),
            dri_feat: 0,
            dev_feat_sel: 0,
            dri_feat_sel: 0,
            qsel: 0,
            cfg: Box::new(net_cfg),
            vqs,
            isr,
            status: 0,
            cfg_gen: 0,
            irq,
        }
    }
}

impl VirtioMmioDev {
    pub fn new_vmnet(
        addr: usize,
        irq: u32,
        name: String,
        irq_sender: Sender<u32>,
        gpa2hva: AddressConverter,
    ) -> Self {
        let vmnet = VirtioDevice::new_vmnet(name, irq, irq_sender, gpa2hva);
        VirtioMmioDev { addr, dev: vmnet }
    }
}
