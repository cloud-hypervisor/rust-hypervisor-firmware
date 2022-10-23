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

use atomic_refcell::AtomicRefCell;

#[cfg(target_arch = "x86_64")]
use uart_16550::SerialPort;

// We use COM1 as it is the standard first serial port.
#[cfg(target_arch = "x86_64")]
pub static PORT: AtomicRefCell<SerialPort> = AtomicRefCell::new(unsafe { SerialPort::new(0x3f8) });

pub struct Serial;
impl fmt::Write for Serial {
    #[cfg(target_arch = "x86_64")]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        PORT.borrow_mut().write_str(s)
    }

    #[cfg(target_arch = "aarch64")]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        todo!();
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        #[cfg(all(feature = "log-serial", not(test)))]
        writeln!($crate::serial::Serial, $($arg)*).unwrap();
        #[cfg(all(feature = "log-serial", test))]
        println!($($arg)*);
    }};
}
