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

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports))]

use core::panic::PanicInfo;

use cpuio::Port;

mod block;
mod bzimage;
mod fat;
mod mem;
mod part;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[cfg(not(test))]
/// Output the message provided in `message` over the serial port
fn serial_message(message: &str) {
    let mut serial: Port<u8> = unsafe { Port::new(0x3f8) };
    for c in message.chars() {
        serial.write(c as u8);
    }
}

#[cfg(not(test))]
/// Reset the VM via the keyboard controller
fn i8042_reset() -> ! {
    loop {
        let mut good: u8 = 0x02;
        let mut i8042_command: Port<u8> = unsafe { Port::new(0x64) };
        while good & 0x02 > 0 {
            good = i8042_command.read();
        }
        i8042_command.write(0xFE);
    }
}

#[cfg(not(test))]
/// Setup page tables to provide an identity mapping over the full 4GiB range
fn setup_pagetables() {
    let pte = mem::MemoryRegion::new(0xb000, 2048 * 8);
    for i in 0..2048 {
        pte.io_write_u64(i * 8, (i << 21) + 0x83u64)
    }

    let pde = mem::MemoryRegion::new(0xa000, 4096);
    for i in 0..4 {
        pde.io_write_u64(i * 8, (0xb000u64 + (0x1000u64 * i)) | 0x03);
    }

    serial_message("Page tables setup\n");
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_message("Starting..\n");

    setup_pagetables();

    let mut device = block::VirtioMMIOBlockDevice::new(0xd0000000u64);
    match device.init() {
        Err(_) => {
            serial_message("Error configuring block device\n");
            i8042_reset();
        }
        Ok(_) => serial_message("Virtio block device configured\n"),
    }

    let mut f;
    match part::find_efi_partition(&mut device) {
        Ok((start, end)) => {
            serial_message("Found EFI partition\n");
            f = fat::Filesystem::new(&mut device, start, end);
            if let Err(_) = f.init() {
                serial_message("Failed to create filesystem\n");
                i8042_reset();
            }
        }
        Err(_) => {
            serial_message("Failed to find EFI partition\n");
            i8042_reset();
        }
    }

    serial_message("Filesystem ready\n");
    let jump_address;

    match f.open("\\EFI\\LINUX\\BZIMAGE") {
        Ok(mut file) => {
            serial_message("Found Linux kernel (bzImage)\n");
            match bzimage::load_kernel(&mut file) {
                Err(_) => {
                    serial_message("Error loading bzImage\n");
                    i8042_reset();
                }
                Ok(addr) => {
                    jump_address = addr;
                    serial_message("Loaded bzimage\n");
                }
            }
        }
        Err(_) => {
            serial_message("Failed to find bzImage\n");
            i8042_reset();
        }
    }

    match f.open("\\EFI\\LINUX\\CMDLINE") {
        Err(fat::Error::NotFound) => {
            serial_message("Skipping loading command line. File not found (CMDLINE).\n")
        }
        Err(_) => serial_message("Error opening CMDLINE file\n"),
        Ok(mut file) => match bzimage::load_commandline(&mut file) {
            Ok(_) => {}
            Err(_) => {
                serial_message("Error loading command line\n");
                i8042_reset();
            }
        },
    }

    // Rely on x86 C calling convention where second argument is put into %rsi register
    let ptr = jump_address as *const ();
    let code: extern "C" fn(u64, u64) = unsafe { core::mem::transmute(ptr) };
    (code)(0 /* dummy value */, bzimage::ZERO_PAGE_START as u64);

    i8042_reset()
}
