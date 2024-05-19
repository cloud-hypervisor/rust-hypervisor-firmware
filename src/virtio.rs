// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

/// Virtio related errors
#[derive(Debug)]
pub enum Error {
    UnsupportedDevice,
    LegacyOnly,
    FeatureNegotiationFailed,
    QueueTooSmall,
}

/// Trait to allow separation of transport from block driver
pub trait VirtioTransport {
    fn init(&mut self, device_type: u32) -> Result<(), Error>;
    fn get_status(&self) -> u32;
    fn set_status(&self, status: u32);
    fn add_status(&self, status: u32);
    #[allow(dead_code)]
    fn reset(&self);
    fn get_features(&self) -> u64;
    fn set_features(&self, features: u64);
    fn set_queue(&self, queue: u16);
    fn get_queue_max_size(&self) -> u16;
    fn set_queue_size(&self, queue_size: u16);
    fn set_descriptors_address(&self, address: u64);
    fn set_avail_ring(&self, address: u64);
    fn set_used_ring(&self, address: u64);
    fn set_queue_enable(&self);
    fn notify_queue(&self, queue: u16);
    fn read_device_config(&self, offset: u64) -> u32;
}
