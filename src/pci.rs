// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use atomic_refcell::AtomicRefCell;

use log::{info, warn};
#[cfg(target_arch = "x86_64")]
use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::{
    mem,
    virtio::{Error as VirtioError, VirtioTransport},
};

const MAX_BUSES: u8 = 8;
const MAX_DEVICES: u8 = 32;
const MAX_FUNCTIONS: u8 = 8;

const INVALID_VENDOR_ID: u16 = 0xffff;

static PCI_CONFIG: AtomicRefCell<PciConfig> = AtomicRefCell::new(PciConfig::new());

#[cfg(not(target_arch = "x86_64"))]
struct PciConfig {
    region: Option<mem::MemoryRegion>,
}

#[cfg(target_arch = "x86_64")]
struct PciConfig {
    address_port: PortWriteOnly<u32>,
    data_port: Port<u32>,
}

impl PciConfig {
    #[cfg(not(target_arch = "x86_64"))]
    const fn new() -> Self {
        // We use Enhanced Configuration Access Mechanism (ECAM).
        Self { region: None }
    }

    #[cfg(target_arch = "x86_64")]
    const fn new() -> Self {
        // We use the legacy, port-based Configuration Access Mechanism (CAM).
        Self {
            address_port: PortWriteOnly::new(0xcf8),
            data_port: Port::new(0xcfc),
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn init(&mut self, base: u64, length: u64) {
        self.region = Some(mem::MemoryRegion::new(base, length));
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn read_at(&mut self, addr: u32) -> u32 {
        self.region
            .as_ref()
            .expect("PCI config space is not initialized")
            .io_read_u32(addr as u64)
    }

    #[cfg(target_arch = "x86_64")]
    fn read_at(&mut self, addr: u32) -> u32 {
        let addr = addr | 1u32 << 31; // enable bit 31

        // SAFETY: We have exclusive access to the ports, so the data read will
        // correspond to the address written.
        unsafe {
            self.address_port.write(addr);
            self.data_port.read()
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn write_at(&mut self, addr: u32, value: u32) {
        self.region
            .as_ref()
            .expect("PCI config space is not initialized")
            .io_write_u32(addr as u64, value)
    }

    #[cfg(target_arch = "x86_64")]
    fn write_at(&mut self, addr: u32, value: u32) {
        let addr = addr | 1u32 << 31; // enable bit 31

        // SAFETY: We have exclusive access to the ports, so the data read will
        // correspond to the address written.
        unsafe {
            self.address_port.write(addr);
            self.data_port.write(value);
        }
    }

    // This is the logic for calculating I/O port address
    #[cfg(target_arch = "x86_64")]
    fn calculate_pci_address(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
        assert_eq!(offset % 4, 0);
        assert!(bus < MAX_BUSES);
        assert!(device < MAX_DEVICES);
        assert!(func < MAX_FUNCTIONS);

        let mut addr = 0;
        addr |= u32::from(bus) << 16; // bus bits 23-16
        addr |= u32::from(device) << 11; // slot/device bits 15-11
        addr |= u32::from(func) << 8; // function bits 10-8
        addr |= u32::from(offset & 0xfc); // register 7-0

        addr
    }

    // This is the logic for calculating PCI ECAM
    #[cfg(not(target_arch = "x86_64"))]
    fn calculate_pci_address(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
        assert_eq!(offset % 4, 0);
        assert!(bus < MAX_BUSES);
        assert!(device < MAX_DEVICES);
        assert!(func < MAX_FUNCTIONS);

        let mut addr = 0;
        addr |= u32::from(bus) << 20; // bus bits 20-27
        addr |= u32::from(device) << 15; // slot/device bits 15-19
        addr |= u32::from(func) << 12; // function bits 12-14
        addr |= offset as u32 & 0x3ff; // register

        addr
    }

    fn read(&mut self, bus: u8, device: u8, func: u8, offset: u8) -> u32 {
        let addr = Self::calculate_pci_address(bus, device, func, offset);
        self.read_at(addr)
    }

    fn write(&mut self, bus: u8, device: u8, func: u8, offset: u8, value: u32) {
        let addr = Self::calculate_pci_address(bus, device, func, offset);
        self.write_at(addr, value);
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init(base: u64, length: u64) {
    PCI_CONFIG.borrow_mut().init(base, length);
}

fn get_device_details(bus: u8, device: u8, func: u8) -> (u16, u16) {
    let data = PCI_CONFIG.borrow_mut().read(bus, device, func, 0);
    ((data & 0xffff) as u16, (data >> 16) as u16)
}

pub fn print_bus() {
    for device in 0..MAX_DEVICES {
        let (vendor_id, device_id) = get_device_details(0, device, 0);
        if vendor_id == INVALID_VENDOR_ID {
            continue;
        }
        info!(
            "Found PCI device vendor={:x} device={:x} in slot={}",
            vendor_id, device_id, device
        );
    }
}

pub fn with_devices<F>(target_vendor_id: u16, target_device_id: u16, mut per_device: F)
where
    F: FnMut(PciDevice) -> bool,
{
    for device in 0..MAX_DEVICES {
        let (vendor_id, device_id) = get_device_details(0, device, 0);
        if vendor_id == target_vendor_id
            && device_id == target_device_id
            && per_device(PciDevice::new(0, device, 0))
        {
            break;
        }
    }
}

fn naturally_align(address: u64, size: u64) -> u64 {
    ((address + size - 1) / size) * size
}

#[derive(Default)]
pub struct PciDevice {
    bus: u8,
    device: u8,
    func: u8,
    bars: [PciBar; 6],
    vendor_id: u16,
    device_id: u16,
}

#[derive(Debug, Default, Clone, Copy)]
enum PciBarType {
    #[default]
    Unused,
    MemorySpace32,
    MemorySpace64,
    IoSpace,
}

#[derive(Default, Clone, Copy)]
struct PciBar {
    bar_type: PciBarType,
    address: u64,
    size: u64,
}

impl PciDevice {
    fn new(bus: u8, device: u8, func: u8) -> PciDevice {
        PciDevice {
            bus,
            device,
            func,
            ..Default::default()
        }
    }

    fn read_u8(&self, offset: u8) -> u8 {
        let offset32 = offset & 0b1111_1100;
        let shift32 = offset & 0b0000_0011;

        let data = self.read_u32(offset32);
        (data >> (shift32 * 8)) as u8
    }

    fn read_u16(&self, offset: u8) -> u16 {
        assert_eq!(offset % 2, 0);
        let offset32 = offset & 0b1111_1100;
        let shift32 = offset & 0b0000_0011;

        let data = self.read_u32(offset32);
        (data >> (shift32 * 8)) as u16
    }

    fn read_u32(&self, offset: u8) -> u32 {
        PCI_CONFIG
            .borrow_mut()
            .read(self.bus, self.device, self.func, offset)
    }

    fn write_u32(&self, offset: u8, value: u32) {
        PCI_CONFIG
            .borrow_mut()
            .write(self.bus, self.device, self.func, offset, value)
    }

    pub fn init(&mut self) {
        let (vendor_id, device_id) = get_device_details(self.bus, self.device, self.func);

        self.vendor_id = vendor_id;
        self.device_id = device_id;

        info!(
            "PCI Device: {}:{}.{} {:x}:{:x}",
            self.bus, self.device, self.func, self.vendor_id, self.device_id
        );

        // Enable responses in memory space
        self.write_u32(0x4, self.read_u32(0x4) | 0x2);

        let mut current_bar_offset = 0x10;
        let mut current_bar = 0;

        //0x24 offset is last bar
        while current_bar_offset <= 0x24 {
            #[allow(clippy::disallowed_names)]
            let bar = self.read_u32(current_bar_offset);

            // lsb is 1 for I/O space bars
            if bar & 1 == 1 {
                if cfg!(target_arch = "x86_64") {
                    self.bars[current_bar].bar_type = PciBarType::IoSpace;
                    self.bars[current_bar].address = u64::from(bar & 0xffff_fffc);
                    self.write_u32(current_bar_offset, 0xffff_ffff);
                    let size = !(self.read_u32(current_bar_offset) & 0xffff_fff0) + 1;
                    self.bars[current_bar].size = u64::from(size);
                    self.write_u32(current_bar_offset, bar);
                } else {
                    panic!("I/O BARs are not supported on this platform");
                }
            } else {
                // bits 2-1 are the type 0 is 32-bit, 2 is 64 bit
                match bar >> 1 & 3 {
                    0 => {
                        self.bars[current_bar].bar_type = PciBarType::MemorySpace32;
                        self.bars[current_bar].address = u64::from(bar & 0xffff_fff0);

                        self.write_u32(current_bar_offset, 0xffff_ffff);
                        let size = (!(self.read_u32(current_bar_offset) & 0xffff_fff0))
                            .checked_add(1)
                            .unwrap_or(0);
                        self.bars[current_bar].size = u64::from(size);
                        self.write_u32(current_bar_offset, bar);
                    }
                    2 => {
                        self.bars[current_bar].bar_type = PciBarType::MemorySpace64;
                        self.bars[current_bar].address = u64::from(bar & 0xffff_fff0);

                        self.write_u32(current_bar_offset, 0xffff_ffff);
                        let lo_size = self.read_u32(current_bar_offset) & 0xffff_fff0;
                        self.write_u32(current_bar_offset, bar);

                        current_bar_offset += 4;

                        #[allow(clippy::disallowed_names)]
                        let bar = self.read_u32(current_bar_offset);
                        self.bars[current_bar].address += u64::from(bar) << 32;

                        self.write_u32(current_bar_offset, 0xffff_ffff);
                        let hi_size = self.read_u32(current_bar_offset);
                        self.write_u32(current_bar_offset, bar);

                        let size = !(u64::from(lo_size) | u64::from(hi_size) << 32) + 1;
                        self.bars[current_bar].size = size;
                    }
                    _ => panic!("Unsupported BAR type"),
                }
            }

            current_bar += 1;
            current_bar_offset += 4;
        }

        #[allow(clippy::disallowed_names)]
        for bar in &self.bars {
            info!(
                "Bar: type={:?} address=0x{:x} size=0x{:x}",
                bar.bar_type, bar.address, bar.size
            );
        }
    }

    pub fn allocate_bars(&mut self, start_address: Option<u64>) -> Option<u64> {
        let mut next_address = start_address;

        let mut current_bar_offset = 0x10;
        let mut current_bar = 0;

        //0x24 offset is last bar
        while current_bar_offset <= 0x24 {
            let bar = self.bars[current_bar];
            if bar.size != 0 && bar.address == 0 {
                if let Some(next_address) = next_address.as_mut() {
                    match bar.bar_type {
                        PciBarType::IoSpace | PciBarType::Unused => {}
                        PciBarType::MemorySpace32 => {
                            let address = naturally_align(*next_address, bar.size);
                            self.write_u32(current_bar_offset, (address).try_into().unwrap());
                            self.bars[current_bar].address = address;
                            *next_address = address + bar.size;
                        }
                        PciBarType::MemorySpace64 => {
                            let address = naturally_align(*next_address, bar.size);
                            self.write_u32(
                                current_bar_offset,
                                (address & 0xffff_ffff).try_into().unwrap(),
                            );
                            current_bar_offset += 4;
                            self.write_u32(current_bar_offset, (address >> 32).try_into().unwrap());
                            self.bars[current_bar].address = address;
                            *next_address = address + bar.size;
                        }
                    }
                } else {
                    panic!("Zero BAR address and no allocation available to allocate from")
                }
            }

            current_bar += 1;
            current_bar_offset += 4;
        }

        #[allow(clippy::disallowed_names)]
        for bar in &self.bars {
            info!(
                "Updated BARs: type={:?} address={:x} size={:x}",
                bar.bar_type, bar.address, bar.size
            );
        }

        next_address
    }
}

#[allow(clippy::enum_variant_names)]
enum VirtioPciCapabilityType {
    CommonConfig = 1,
    NotifyConfig = 2,
    #[allow(unused)]
    IsrConfig = 3,
    DeviceConfig = 4,
    #[allow(unused)]
    PciConfig = 5,
}

#[derive(Default)]
pub struct VirtioPciTransport {
    device: PciDevice,
    region: mem::MemoryRegion,               // common configuration region
    notify_region: mem::MemoryRegion,        // notify region
    notify_off_multiplier: u32,              // from notify config cap
    device_config_region: mem::MemoryRegion, // device specific region
}

impl VirtioPciTransport {
    pub fn new(device: PciDevice) -> VirtioPciTransport {
        VirtioPciTransport {
            device,
            ..Default::default()
        }
    }
}
// Common Configuration registers:
/// le32 device_feature_select;     // 0x00 // read-write
/// le32 device_feature;            // 0x04 // read-only for driver
/// le32 driver_feature_select;     // 0x08 // read-write
/// le32 driver_feature;            // 0x0C // read-write
/// le16 msix_config;               // 0x10 // read-write
/// le16 num_queues;                // 0x12 // read-only for driver
/// u8 device_status;               // 0x14 // read-write (driver_status)
/// u8 config_generation;           // 0x15 // read-only for driver
/// ** About a specific virtqueue.
/// le16 queue_select;              // 0x16 // read-write
/// le16 queue_size;                // 0x18 // read-write, power of 2, or 0.
/// le16 queue_msix_vector;         // 0x1A // read-write
/// le16 queue_enable;              // 0x1C // read-write (Ready)
/// le16 queue_notify_off;          // 0x1E // read-only for driver
/// le64 queue_desc;                // 0x20 // read-write
/// le64 queue_avail;               // 0x28 // read-write
/// le64 queue_used;                // 0x30 // read-write

impl VirtioTransport for VirtioPciTransport {
    fn init(&mut self, _device_type: u32) -> Result<(), VirtioError> {
        // Read status register
        let status = self.device.read_u16(0x06);

        // bit 4 of status is capability bit
        if status & 1 << 4 == 0 {
            warn!("No capabilities detected");
            return Err(VirtioError::UnsupportedDevice);
        }

        // capabilities list offset is at 0x34
        let mut cap_next = self.device.read_u8(0x34);

        while cap_next < 0xff && cap_next > 0 {
            // vendor specific capability
            if self.device.read_u8(cap_next) == 0x09 {
                // These offsets are into the following structure:
                // struct virtio_pci_cap {
                //         u8 cap_vndr;    /* Generic PCI field: PCI_CAP_ID_VNDR */
                //         u8 cap_next;    /* Generic PCI field: next ptr. */
                //         u8 cap_len;     /* Generic PCI field: capability length */
                //         u8 cfg_type;    /* Identifies the structure. */
                //         u8 bar;         /* Where to find it. */
                //         u8 padding[3];  /* Pad to full dword. */
                //         le32 offset;    /* Offset within bar. */
                //         le32 length;    /* Length of the structure, in bytes. */
                // };
                let cfg_type = self.device.read_u8(cap_next + 3);
                #[allow(clippy::disallowed_names)]
                let bar = self.device.read_u8(cap_next + 4);
                let offset = self.device.read_u32(cap_next + 8);
                let length = self.device.read_u32(cap_next + 12);

                if cfg_type == VirtioPciCapabilityType::CommonConfig as u8 {
                    self.region = mem::MemoryRegion::new(
                        self.device.bars[usize::from(bar)].address + u64::from(offset),
                        u64::from(length),
                    );
                }

                if cfg_type == VirtioPciCapabilityType::NotifyConfig as u8 {
                    self.notify_region = mem::MemoryRegion::new(
                        self.device.bars[usize::from(bar)].address + u64::from(offset),
                        u64::from(length),
                    );

                    // struct virtio_pci_notify_cap {
                    //         struct virtio_pci_cap cap;
                    //         le32 notify_off_multiplier; /* Multiplier for queue_notify_off. */
                    // };
                    self.notify_off_multiplier = self.device.read_u32(cap_next + 16);
                }

                if cfg_type == VirtioPciCapabilityType::DeviceConfig as u8 {
                    self.device_config_region = mem::MemoryRegion::new(
                        self.device.bars[usize::from(bar)].address + u64::from(offset),
                        u64::from(length),
                    );
                }
            }
            cap_next = self.device.read_u8(cap_next + 1)
        }

        Ok(())
    }

    fn get_status(&self) -> u32 {
        // device_status: 0x14
        u32::from(self.region.io_read_u8(0x14))
    }

    fn set_status(&self, value: u32) {
        // device_status: 0x14
        self.region.io_write_u8(0x14, value as u8);
    }

    fn add_status(&self, value: u32) {
        self.set_status(self.get_status() | value);
    }

    fn reset(&self) {
        self.set_status(0);
    }

    fn get_features(&self) -> u64 {
        // device_feature_select: 0x00
        self.region.io_write_u32(0x00, 0);
        // device_feature: 0x04
        let mut device_features: u64 = u64::from(self.region.io_read_u32(0x04));
        // device_feature_select: 0x00
        self.region.io_write_u32(0x00, 1);
        // device_feature: 0x04
        device_features |= u64::from(self.region.io_read_u32(0x04)) << 32;

        device_features
    }

    fn set_features(&self, features: u64) {
        // driver_feature_select: 0x08
        self.region.io_write_u32(0x08, 0);
        // driver_feature: 0x0c
        self.region.io_write_u32(0x0c, features as u32);
        // driver_feature_select: 0x08
        self.region.io_write_u32(0x08, 1);
        // driver_feature: 0x0c
        self.region.io_write_u32(0x0c, (features >> 32) as u32);
    }

    fn set_queue(&self, queue: u16) {
        // queue_select: 0x16
        self.region.io_write_u16(0x16, queue);
    }

    fn get_queue_max_size(&self) -> u16 {
        // queue_size: 0x18
        self.region.io_read_u16(0x18)
    }

    fn set_queue_size(&self, queue_size: u16) {
        // queue_size: 0x18
        self.region.io_write_u16(0x18, queue_size);
    }

    fn set_descriptors_address(&self, addr: u64) {
        // queue_desc: 0x20
        self.region.io_write_u32(0x20, (addr & 0xffff_ffff) as u32);
        self.region.io_write_u32(0x20 + 4, (addr >> 32) as u32);
    }

    fn set_avail_ring(&self, addr: u64) {
        // queue_avail: 0x28
        self.region.io_write_u32(0x28, (addr & 0xffff_ffff) as u32);
        self.region.io_write_u32(0x28 + 4, (addr >> 32) as u32);
    }

    fn set_used_ring(&self, addr: u64) {
        // queue_used: 0x30
        self.region.io_write_u32(0x30, (addr & 0xffff_ffff) as u32);
        self.region.io_write_u32(0x30 + 4, (addr >> 32) as u32);
    }

    fn set_queue_enable(&self) {
        // queue_enable: 0x1c
        self.region.io_write_u16(0x1c, 0x1);
    }

    fn notify_queue(&self, queue: u16) {
        // queue_notify_off: 0x1e
        let queue_notify_off = self.region.io_read_u16(0x1e);

        self.notify_region.io_write_u32(
            u64::from(queue_notify_off) * u64::from(self.notify_off_multiplier),
            u32::from(queue),
        );
    }

    fn read_device_config(&self, offset: u64) -> u32 {
        self.device_config_region.io_read_u32(offset)
    }
}
