// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Rivos Inc.

use crate::mem::MemoryRegion;
use core::fmt;

pub struct UartMmio {
    region: MemoryRegion,
}

impl UartMmio {
    pub const fn new(base: u64) -> UartMmio {
        UartMmio {
            region: MemoryRegion::new(base, 8),
        }
    }

    fn send(&mut self, byte: u8) {
        self.region.io_write_u8(0, byte)
    }

    pub fn init(&mut self) {}
}

impl fmt::Write for UartMmio {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}
