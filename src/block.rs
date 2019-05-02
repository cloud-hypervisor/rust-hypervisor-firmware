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

use core::cell::RefCell;

#[cfg(not(test))]
use crate::virtio::Error as VirtioError;
#[cfg(not(test))]
use crate::virtio::VirtioTransport;

#[cfg(not(test))]
const QUEUE_SIZE: usize = 16;

#[repr(C)]
#[repr(align(16))]
#[derive(Default)]
#[cfg(not(test))]
/// A virtio qeueue entry descriptor
struct Desc {
    addr: u64,
    length: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[repr(align(2))]
#[derive(Default)]
#[cfg(not(test))]
/// The virtio available ring
struct AvailRing {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
}

#[repr(C)]
#[repr(align(4))]
#[derive(Default)]
#[cfg(not(test))]
/// The virtio used ring
struct UsedRing {
    flags: u16,
    idx: u16,
    ring: [UsedElem; QUEUE_SIZE],
}

#[repr(C)]
#[derive(Default)]
#[cfg(not(test))]
/// A single element in the used ring
struct UsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
#[repr(align(64))]
#[cfg(not(test))]
/// Device driver for virtio block over any transport
pub struct VirtioBlockDevice<'a> {
    transport: &'a mut VirtioTransport,
    state: RefCell<DriverState>,
}

#[repr(C)]
#[repr(align(64))]
#[derive(Default)]
#[cfg(not(test))]
struct DriverState {
    descriptors: [Desc; QUEUE_SIZE],
    avail: AvailRing,
    used: UsedRing,
    next_head: usize,
}

pub enum Error {
    BlockIOError,

    #[cfg(not(test))]
    BlockNotSupported,
}

#[repr(C)]
#[cfg(not(test))]
/// Header used for virtio block requests
struct BlockRequestHeader {
    request: u32,
    reserved: u32,
    sector: u64,
}

#[repr(C)]
#[cfg(not(test))]
/// Footer used for virtio block requests
struct BlockRequestFooter {
    status: u8,
}

pub trait SectorRead {
    /// Read a single sector (512 bytes) from the block device. `data` must be
    /// exactly 512 bytes long.
    fn read(&self, sector: u64, data: &mut [u8]) -> Result<(), Error>;
}

#[cfg(not(test))]
impl<'a> VirtioBlockDevice<'a> {
    pub fn new(transport: &'a mut VirtioTransport) -> VirtioBlockDevice<'a> {
        VirtioBlockDevice {
            transport,
            state: RefCell::new(DriverState::default()),
        }
    }

    pub fn reset(&self) {
        self.transport.reset()
    }

    pub fn init(&mut self) -> Result<(), VirtioError> {
        const VIRTIO_SUBSYSTEM_BLOCK: u32 = 0x2;
        const VIRTIO_F_VERSION_1: u64 = 1 << 32;

        const VIRTIO_STATUS_RESET: u32 = 0;
        const VIRTIO_STATUS_ACKNOWLEDGE: u32 = 1;
        const VIRTIO_STATUS_DRIVER: u32 = 2;
        const VIRTIO_STATUS_FEATURES_OK: u32 = 8;
        const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
        const VIRTIO_STATUS_FAILED: u32 = 128;

        // Initialise the transport
        self.transport.init(VIRTIO_SUBSYSTEM_BLOCK)?;

        // Reset device
        self.transport.set_status(VIRTIO_STATUS_RESET);

        // Acknowledge
        self.transport.add_status(VIRTIO_STATUS_ACKNOWLEDGE);

        // And advertise driver
        self.transport.add_status(VIRTIO_STATUS_DRIVER);

        // Request device features
        let device_features = self.transport.get_features();

        if device_features & VIRTIO_F_VERSION_1 != VIRTIO_F_VERSION_1 {
            self.transport.add_status(VIRTIO_STATUS_FAILED);
            return Err(VirtioError::VirtioLegacyOnly);
        }

        // Report driver features
        self.transport.set_features(device_features);

        self.transport.add_status(VIRTIO_STATUS_FEATURES_OK);
        if self.transport.get_status() & VIRTIO_STATUS_FEATURES_OK != VIRTIO_STATUS_FEATURES_OK {
            self.transport.add_status(VIRTIO_STATUS_FAILED);
            return Err(VirtioError::VirtioFeatureNegotiationFailed);
        }

        // Program queues
        self.transport.set_queue(0);

        let max_queue = self.transport.get_queue_max_size();

        // Hardcoded queue size to QUEUE_SIZE at the moment
        if max_queue < QUEUE_SIZE as u16 {
            self.transport.add_status(VIRTIO_STATUS_FAILED);
            return Err(VirtioError::VirtioQueueTooSmall);
        }
        self.transport.set_queue_size(QUEUE_SIZE as u16);

        // Update all queue parts
        let state = self.state.borrow_mut();
        let addr = state.descriptors.as_ptr() as u64;
        self.transport.set_descriptors_address(addr);

        let addr = (&state.avail as *const _) as u64;
        self.transport.set_avail_ring(addr);

        let addr = (&state.used as *const _) as u64;
        self.transport.set_used_ring(addr);

        // Confirm queue
        self.transport.set_queue_enable();

        // Report driver ready
        self.transport.add_status(VIRTIO_STATUS_DRIVER_OK);

        Ok(())
    }
}

#[cfg(not(test))]
impl<'a> SectorRead for VirtioBlockDevice<'a> {
    fn read(&self, sector: u64, data: &mut [u8]) -> Result<(), Error> {
        assert_eq!(512, data.len());

        const VIRTQ_DESC_F_NEXT: u16 = 1;
        const VIRTQ_DESC_F_WRITE: u16 = 2;

        const VIRTIO_BLK_S_OK: u8 = 0;
        const VIRTIO_BLK_S_IOERR: u8 = 1;
        const VIRTIO_BLK_S_UNSUPP: u8 = 2;

        let header = BlockRequestHeader {
            request: 0,
            reserved: 0,
            sector,
        };

        let footer = BlockRequestFooter { status: 0 };

        let mut state = self.state.borrow_mut();

        let next_head = state.next_head;
        let mut d = &mut state.descriptors[next_head];
        let next_desc = (next_head + 1) % QUEUE_SIZE;
        d.addr = (&header as *const _) as u64;
        d.length = core::mem::size_of::<BlockRequestHeader>() as u32;
        d.flags = VIRTQ_DESC_F_NEXT;
        d.next = next_desc as u16;

        let mut d = &mut state.descriptors[next_desc];
        let next_desc = (next_desc + 1) % QUEUE_SIZE;
        d.addr = data.as_ptr() as u64;
        d.length = core::mem::size_of::<[u8; 512]>() as u32;
        d.flags = VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE;
        d.next = next_desc as u16;

        let mut d = &mut state.descriptors[next_desc];
        d.addr = (&footer as *const _) as u64;
        d.length = core::mem::size_of::<BlockRequestFooter>() as u32;
        d.flags = VIRTQ_DESC_F_WRITE;
        d.next = 0;

        // Update ring to point to head of chain. Fence. Then update idx
        let avail_index = state.avail.idx;
        state.avail.ring[(avail_index % QUEUE_SIZE as u16) as usize] = state.next_head as u16;
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

        state.avail.idx = state.avail.idx.wrapping_add(1);

        // Next free descriptor to use
        state.next_head = (next_desc + 1) % QUEUE_SIZE;

        // Notify queue has been updated
        self.transport.notify_queue(0);

        // Check for the completion of the request
        while unsafe { core::ptr::read_volatile(&state.used.idx) } != state.avail.idx {
            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
        }

        match footer.status {
            VIRTIO_BLK_S_OK => Ok(()),
            VIRTIO_BLK_S_IOERR => Err(Error::BlockIOError),
            VIRTIO_BLK_S_UNSUPP => Err(Error::BlockNotSupported),
            _ => Err(Error::BlockNotSupported),
        }
    }
}
