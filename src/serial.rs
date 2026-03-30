// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

// Inspired by https://github.com/phil-opp/blog_os/blob/post-03/src/vga_buffer.rs
// from Philipp Oppermann

use core::fmt;

use atomic_refcell::AtomicRefCell;

#[cfg(target_arch = "aarch64")]
use crate::{arch::aarch64::layout::map, uart_pl011::Pl011 as UartPl011};

#[cfg(target_arch = "x86_64")]
use uart_16550::{backend::PioBackend, Config, Uart16550Tty};

#[cfg(target_arch = "riscv64")]
use crate::uart_mmio::UartMmio;

// We use COM1 as it is the standard first serial port.
#[cfg(target_arch = "x86_64")]
pub static PORT: AtomicRefCell<Option<Uart16550Tty<PioBackend>>> = AtomicRefCell::new(None);

#[cfg(target_arch = "aarch64")]
pub static PORT: AtomicRefCell<UartPl011> =
    AtomicRefCell::new(UartPl011::new(map::mmio::PL011_START));

// TODO: Fill from FDT?
#[cfg(target_arch = "riscv64")]
const SERIAL_PORT_ADDRESS: u64 = 0x1000_0000;
#[cfg(target_arch = "riscv64")]
pub static PORT: AtomicRefCell<UartMmio> = AtomicRefCell::new(UartMmio::new(SERIAL_PORT_ADDRESS));

#[cfg(target_arch = "x86_64")]
pub fn init() {
    let mut port = PORT.borrow_mut();

    if port.is_none() {
        *port = Some(unsafe {
            Uart16550Tty::new_port(0x3f8, Config::default())
                .expect("Failed to initialize UART16550")
        });
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init() {
    PORT.borrow_mut().init();
}

pub struct Serial;
impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        #[cfg(target_arch = "x86_64")]
        {
            let mut port = PORT.borrow_mut();

            if let Some(port) = port.as_mut() {
                return fmt::Write::write_str(port, s);
            }

            Ok(())
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            PORT.borrow_mut().write_str(s)
        }
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
