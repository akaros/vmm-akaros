/* SPDX-License-Identifier: GPL-2.0-only */

use std::collections::HashMap;

pub struct ConfigAddr(pub u32);

impl ConfigAddr {
    pub fn enabled(&self) -> bool {
        (self.0 & (1 << 31)) != 0
    }

    pub fn offset(&self) -> u8 {
        (self.0 & 0xff) as u8
    }

    pub fn bdf(&self) -> u16 {
        ((self.0 >> 8) & 0xffff) as u16
    }
}

pub trait PciDevice {
    fn read(&self, offset: u8) -> u32;
    fn write(&mut self, offset: u8, value: u32);
}

pub struct HostBridge {
    pub data: [u32; 16],
}

impl HostBridge {
    pub fn new() -> Self {
        //0:00.0 Host bridge: Intel Corporation 440BX/ZX/DX - 82443BX/ZX/DX Host bridge (rev 01)
        let mut data = [0; 16];
        data[0] = 0x71908086;
        data[1] = 0x02000006;
        data[2] = 0x06000001;
        HostBridge { data }
    }
}

impl PciDevice for HostBridge {
    fn read(&self, offset: u8) -> u32 {
        self.data[(offset >> 2) as usize]
    }

    fn write(&mut self, offset: u8, value: u32) {
        self.data[(offset >> 2) as usize] = value;
    }
}

pub struct PciBus {
    pub config_addr: ConfigAddr,
    pub devices: HashMap<u16, Box<dyn PciDevice + Send>>,
}

impl PciBus {
    pub fn new() -> Self {
        let host_bridge = HostBridge::new();
        let mut pci_bus = PciBus {
            config_addr: ConfigAddr(0),
            devices: HashMap::new(),
        };
        pci_bus.devices.insert(0, Box::new(host_bridge));
        pci_bus
    }

    pub fn read(&self) -> u32 {
        let bdf = self.config_addr.bdf();
        if let Some(device) = self.devices.get(&bdf) {
            device.read(self.config_addr.offset())
        } else {
            u32::MAX
        }
    }

    pub fn write(&mut self, value: u32) {
        let bdf = self.config_addr.bdf();
        if let Some(device) = self.devices.get_mut(&bdf) {
            device.write(self.config_addr.offset(), value);
        }
    }
}
