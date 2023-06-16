// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use core::fmt;

pub struct Pl011 {
    base: usize,
}

impl Pl011 {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    pub fn init(&mut self) {
        // Do nothing
    }

    pub fn send(&mut self, data: u8) {
        unsafe {
            core::ptr::write_volatile(self.base as *mut u8, data);
        }
    }
}

impl fmt::Write for Pl011 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // Unix-like OS treats LF as CRLF
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
        Ok(())
    }
}
