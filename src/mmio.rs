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

use crate::mem;

use crate::virtio::Error as VirtioError;
use crate::virtio::VirtioTransport;

pub struct VirtioMMIOTransport {
    region: mem::MemoryRegion,
}

impl VirtioMMIOTransport {
    pub fn new(base: u64) -> VirtioMMIOTransport {
        VirtioMMIOTransport {
            region: crate::mem::MemoryRegion::new(base, 4096),
        }
    }
}

impl VirtioTransport for VirtioMMIOTransport {
    fn init(&mut self, device_type: u32) -> Result<(), VirtioError> {
        const VIRTIO_MAGIC: u32 = 0x7472_6976;
        const VIRTIO_VERSION: u32 = 0x2;
        if self.region.io_read_u32(0x000) != VIRTIO_MAGIC {
            return Err(VirtioError::VirtioMagicInvalid);
        }

        if self.region.io_read_u32(0x004) != VIRTIO_VERSION {
            return Err(VirtioError::VirtioVersionInvalid);
        }

        if self.region.io_read_u32(0x008) != device_type {
            return Err(VirtioError::VirtioUnsupportedDevice);
        }
        Ok(())
    }

    fn get_status(&self) -> u32 {
        self.region.io_read_u32(0x70)
    }

    fn set_status(&self, value: u32) {
        self.region.io_write_u32(0x70, value);
    }

    fn add_status(&self, value: u32) {
        self.set_status(self.get_status() | value);
    }

    fn reset(&self) {
        self.set_status(0);
    }

    fn get_features(&self) -> u64 {
        self.region.io_write_u32(0x014, 0);
        let mut device_features: u64 = u64::from(self.region.io_read_u32(0x010));
        self.region.io_write_u32(0x014, 1);
        device_features |= u64::from(self.region.io_read_u32(0x010)) << 32;

        device_features
    }

    fn set_features(&self, features: u64) {
        self.region.io_write_u32(0x024, 0);
        self.region.io_write_u32(0x020, features as u32);
        self.region.io_write_u32(0x024, 1);
        self.region.io_write_u32(0x020, (features >> 32) as u32);
    }

    fn set_queue(&self, queue: u16) {
        self.region.io_write_u32(0x030, u32::from(queue));
    }

    fn get_queue_max_size(&self) -> u16 {
        self.region.io_read_u32(0x034) as u16
    }

    fn set_queue_size(&self, queue_size: u16) {
        self.region.io_write_u32(0x038, u32::from(queue_size));
    }
    fn set_descriptors_address(&self, addr: u64) {
        self.region.io_write_u32(0x080, addr as u32);
        self.region.io_write_u32(0x084, (addr >> 32) as u32);
    }

    fn set_avail_ring(&self, addr: u64) {
        self.region.io_write_u32(0x090, addr as u32);
        self.region.io_write_u32(0x094, (addr >> 32) as u32);
    }

    fn set_used_ring(&self, addr: u64) {
        self.region.io_write_u32(0x0a0, addr as u32);
        self.region.io_write_u32(0x0a4, (addr >> 32) as u32);
    }
    fn set_queue_enable(&self) {
        self.region.io_write_u32(0x044, 0x1);
    }
    fn notify_queue(&self, queue: u16) {
        self.region.io_write_u32(0x50, u32::from(queue));
    }
    fn read_device_config(&self, offset: u64) -> u32 {
        self.region.io_read_u32(0x100 + offset)
    }
}
