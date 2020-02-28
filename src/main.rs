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

#![feature(global_asm)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(test, allow(unused_imports, dead_code))]
#![cfg_attr(not(feature = "log-serial"), allow(unused_variables, unused_imports))]

use core::panic::PanicInfo;

use x86_64::instructions::hlt;

#[macro_use]
mod serial;

#[macro_use]
mod common;

mod block;
mod bzimage;
mod efi;
mod fat;
mod loader;
mod mem;
mod part;
mod pci;
mod pe;
mod virtio;

#[cfg(not(test))]
global_asm!(include_str!("asm/ram64.s"));

#[cfg(all(not(test), feature = "log-panic"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log!("PANIC: {}", info);
    loop {
        hlt()
    }
}

#[cfg(all(not(test), not(feature = "log-panic")))]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

/// Setup page tables to provide an identity mapping over the full 4GiB range
fn setup_pagetables() {
    type PageTable = [u64; 512];

    extern "C" {
        static pml3t: PageTable;
        static pml2t: PageTable;
        static address_space_gib: u8;
    }
    let num_gib = unsafe { &address_space_gib } as *const _ as usize as u64;
    log!("Setting up {} GiB identity mapping", num_gib);

    let pml2t_addr = unsafe { pml2t.as_ptr() } as usize as u64;
    let pte = mem::MemoryRegion::new(pml2t_addr, num_gib * 4096);
    for i in 0..(512 * num_gib) {
        pte.io_write_u64(i * 8, (i << 21) + 0x83u64)
    }

    let pde = mem::MemoryRegion::from_slice(unsafe { &pml3t });
    for i in 0..num_gib {
        pde.io_write_u64(i * 8, (pml2t_addr + (0x1000u64 * i)) | 0x03);
    }

    log!("Page tables setup");
}

const VIRTIO_PCI_VENDOR_ID: u16 = 0x1af4;
const VIRTIO_PCI_BLOCK_DEVICE_ID: u16 = 0x1042;

fn boot_from_device(device: &mut block::VirtioBlockDevice) -> bool {
    match device.init() {
        Err(_) => {
            log!("Error configuring block device");
            return false;
        }
        Ok(_) => log!(
            "Virtio block device configured. Capacity: {} sectors",
            device.get_capacity()
        ),
    }

    let mut f;

    match part::find_efi_partition(device) {
        Ok((start, end)) => {
            log!("Found EFI partition");
            f = fat::Filesystem::new(device, start, end);
            if f.init().is_err() {
                log!("Failed to create filesystem");
                return false;
            }
        }
        Err(_) => {
            log!("Failed to find EFI partition");
            return false;
        }
    }

    log!("Filesystem ready");

    let jump_address;

    match loader::load_default_entry(&f) {
        Ok(addr) => {
            jump_address = addr;
        }
        Err(_) => {
            log!("Error loading default entry. Using EFI boot.");
            match f.open("/EFI/BOOT/BOOTX64 EFI") {
                Ok(mut file) => {
                    log!("Found bootloader (BOOTX64.EFI)");
                    let mut l = pe::Loader::new(&mut file);
                    match l.load(0x20_0000) {
                        Ok((a, size)) => {
                            log!("Executable loaded");
                            efi::efi_exec(a, 0x20_0000, size, &f, device);
                            return true;
                        }
                        Err(e) => {
                            match e {
                                pe::Error::FileError => log!("File error"),
                                pe::Error::InvalidExecutable => log!("Invalid executable"),
                            }
                            return false;
                        }
                    }
                }
                Err(_) => {
                    log!("Failed to find bootloader");
                    return false;
                }
            }
        }
    }

    device.reset();

    log!("Jumping to kernel");

    // Rely on x86 C calling convention where second argument is put into %rsi register
    let ptr = jump_address as *const ();
    let code: extern "C" fn(u64, u64) = unsafe { core::mem::transmute(ptr) };
    (code)(0 /* dummy value */, bzimage::ZERO_PAGE_START as u64);
    true
}

#[cfg_attr(not(test), no_mangle)]
pub extern "C" fn rust64_start() -> ! {
    log!("\nStarting..");
    setup_pagetables();

    pci::print_bus();

    pci::with_devices(
        VIRTIO_PCI_VENDOR_ID,
        VIRTIO_PCI_BLOCK_DEVICE_ID,
        |pci_device| {
            let mut pci_transport = pci::VirtioPciTransport::new(pci_device);
            block::VirtioBlockDevice::new(&mut pci_transport);
            let mut device = block::VirtioBlockDevice::new(&mut pci_transport);
            boot_from_device(&mut device)
        },
    );

    panic!("Unable to boot from any virtio-blk device")
}
