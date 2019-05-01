// Copyright © 2019 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use cpuio::Port;

#[cfg(not(test))]
const CONFIG_ADDRESS: u16 = 0xcf8;
#[cfg(not(test))]
const CONFIG_DATA: u16 = 0xcfc;

#[cfg(not(test))]
const MAX_DEVICES: u8 = 32;
#[cfg(not(test))]
const MAX_FUNCTIONS: u8 = 8;

#[cfg(not(test))]
const INVALID_VENDOR_ID: u16 = 0xffff;

#[cfg(not(test))]
fn pci_config_read_u32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    assert_eq!(offset % 4, 0);
    assert!(device < MAX_DEVICES);
    assert!(func < MAX_FUNCTIONS);

    let addr = u32::from(bus) << 16; // bus bits 23-16
    let addr = addr | u32::from(device) << 11; // slot/device bits 15-11
    let addr = addr | u32::from(func) << 8; // function bits 10-8
    let addr = addr | u32::from(offset & 0xfc); // register 7-0
    let addr = addr | 1u32 << 31; // enable bit 31

    let mut config_address_port: Port<u32> = unsafe { Port::new(CONFIG_ADDRESS) };
    config_address_port.write(addr);

    let mut config_data_port: Port<u32> = unsafe { Port::new(CONFIG_DATA) };

    config_data_port.read()
}

#[cfg(not(test))]
fn pci_config_read_u8(bus: u8, device: u8, func: u8, offset: u8) -> u8 {
    (pci_config_read_u32(bus, device, func, offset & !3) >> ((offset % 4) * 8)) as u8
}

#[cfg(not(test))]
fn pci_config_read_u16(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    assert_eq!(offset % 2, 0);
    (pci_config_read_u32(bus, device, func, offset & !3) >> ((offset % 4) * 8)) as u16
}

#[cfg(not(test))]
fn get_device_details(bus: u8, device: u8, func: u8) -> (u16, u16) {
    (
        pci_config_read_u16(bus, device, func, 0),
        pci_config_read_u16(bus, device, func, 2),
    )
}

#[cfg(not(test))]
pub fn print_bus() {
    for device in 0..MAX_DEVICES {
        let (vendor_id, device_id) = get_device_details(0, device, 0);
        if vendor_id == INVALID_VENDOR_ID {
            continue;
        }
        log!(
            "Found PCI device vendor={:x} device={:x} in slot={}\n",
            vendor_id,
            device_id,
            device
        );
        let mut pci_device = PciDevice::new(0, device, 0);
        pci_device.init();
    }
}

#[cfg(not(test))]
#[derive(Debug)]
enum PciBarType {
    Unused,
    MemorySpace32,
    MemorySpace64,
    IoSpace,
}

#[cfg(not(test))]
impl Default for PciBarType {
    fn default() -> Self {
        PciBarType::Unused
    }
}

#[cfg(not(test))]
#[derive(Default)]
struct PciBar {
    bar_type: PciBarType,
    address: u64,
}

#[cfg(not(test))]
#[derive(Default)]
struct PciDevice {
    bus: u8,
    device: u8,
    func: u8,
    bars: [PciBar; 6],
    vendor_id: u16,
    device_id: u16,
}

#[cfg(not(test))]
impl PciDevice {
    fn new(bus: u8, device: u8, func: u8) -> PciDevice {
        PciDevice {
            bus,
            device,
            func,
            ..Default::default()
        }
    }

    fn config_read_u8(&self, offset: u8) -> u8 {
        pci_config_read_u8(self.bus, self.device, self.func, offset)
    }

    fn config_read_u16(&self, offset: u8) -> u16 {
        pci_config_read_u16(self.bus, self.device, self.func, offset)
    }

    fn config_read_u32(&self, offset: u8) -> u32 {
        pci_config_read_u32(self.bus, self.device, self.func, offset)
    }

    fn init(&mut self) {
        let (vendor_id, device_id) = get_device_details(self.bus, self.device, self.func);

        self.vendor_id = vendor_id;
        self.device_id = device_id;

        log!(
            "PCI Device: {}:{}.{} {:x}:{:x}\n",
            self.bus,
            self.device,
            self.func,
            self.vendor_id,
            self.device_id
        );

        let mut current_bar_offset = 0x10;
        let mut current_bar = 0;

        //0x24 offset is last bar
        while current_bar_offset < 0x24 {
            #[allow(clippy::blacklisted_name)]
            let bar = self.config_read_u32(current_bar_offset);

            // lsb is 1 for I/O space bars
            if bar & 1 == 1 {
                self.bars[current_bar].bar_type = PciBarType::IoSpace;
                self.bars[current_bar].address = u64::from(bar & 0xffff_fffc);
            } else {
                // bits 2-1 are the type 0 is 32-but, 2 is 64 bit
                match bar >> 1 & 3 {
                    0 => {
                        self.bars[current_bar].bar_type = PciBarType::MemorySpace32;
                        self.bars[current_bar].address = u64::from(bar & 0xffff_fff0);
                    }
                    2 => {
                        self.bars[current_bar].bar_type = PciBarType::MemorySpace64;
                        self.bars[current_bar].address = u64::from(bar & 0xffff_fff0);
                        current_bar_offset += 4;

                        #[allow(clippy::blacklisted_name)]
                        let bar = self.config_read_u32(current_bar_offset);
                        self.bars[current_bar].address += u64::from(bar) << 32;
                    }
                    _ => panic!("Unsupported BAR type"),
                }
            }

            current_bar += 1;
            current_bar_offset += 4;
        }

        #[allow(clippy::blacklisted_name)]
        for bar in &self.bars {
            log!("Bar: type={:?} address={:x}\n", bar.bar_type, bar.address);
        }
    }
}
