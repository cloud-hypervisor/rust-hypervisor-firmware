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

// Inspired by https://github.com/phil-opp/blog_os/blob/post-03/src/vga_buffer.rs
// from Philipp Oppermann

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

use cpuio::Port;

lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger {
        port: unsafe { Port::new(0x3f8) }
    });
}

struct Logger {
    port: Port<u8>,
}

impl Logger {
    pub fn write_byte(&mut self, byte: u8) {
        self.port.write(byte)
    }

    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_byte(c as u8);
        }
    }
}

impl fmt::Write for Logger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => ($crate::logger::_log(format_args!($($arg)*)));
}

#[cfg(not(test))]
pub fn _log(args: fmt::Arguments) {
    use core::fmt::Write;
    LOGGER.lock().write_fmt(args).unwrap();
}

#[cfg(test)]
pub fn _log(args: fmt::Arguments) {
    use std::io::{self, Write};
    write!(&mut std::io::stdout(), "{}", args).expect("stdout logging failed");
}
