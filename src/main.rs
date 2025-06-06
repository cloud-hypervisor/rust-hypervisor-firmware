// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

#![feature(asm_const)]
#![feature(exposed_provenance)]
#![feature(slice_take)]
#![feature(stmt_expr_attributes)]
#![feature(strict_provenance)]
#![feature(sync_unsafe_cell)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(
    all(not(test), not(feature = "integration_tests")),
    feature(alloc_error_handler)
)]
#![cfg_attr(test, allow(unused_imports, dead_code))]
#![cfg_attr(not(feature = "log-serial"), allow(unused_variables, unused_imports))]
#![cfg_attr(target_arch = "riscv64", feature(riscv_ext_intrinsics))]

#[cfg(all(not(test), not(feature = "integration_tests")))]
use core::panic::PanicInfo;

use log::{error, info, warn};
#[cfg(all(
    not(test),
    not(feature = "integration_tests"),
    target_arch = "x86_64",
    feature = "log-panic"
))]
use x86_64::instructions::hlt;

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64::layout::code_range;

#[macro_use]
mod serial;

#[macro_use]
mod common;

mod arch;
mod block;
mod boot;
mod bootinfo;
mod bzimage;
#[cfg(target_arch = "x86_64")]
mod cmos;
#[cfg(target_arch = "x86_64")]
mod coreboot;
mod delay;
mod efi;
mod fat;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
mod fdt;
#[cfg(all(test, feature = "integration_tests"))]
mod integration;
mod layout;
mod loader;
mod logger;
mod mem;
mod part;
mod pci;
mod pe;
#[cfg(all(target_arch = "x86_64", not(feature = "coreboot")))]
mod pvh;
mod rtc;
#[cfg(target_arch = "riscv64")]
mod rtc_goldfish;
#[cfg(target_arch = "aarch64")]
mod rtc_pl031;
#[cfg(target_arch = "riscv64")]
mod uart_mmio;
#[cfg(target_arch = "aarch64")]
mod uart_pl011;
mod virtio;

#[cfg(all(not(test), not(feature = "integration_tests"), feature = "log-panic"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log!("PANIC: {}", info);
    loop {
        #[cfg(target_arch = "x86_64")]
        hlt()
    }
}

#[cfg(all(
    not(test),
    not(feature = "integration_tests"),
    not(feature = "log-panic")
))]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

const VIRTIO_PCI_VENDOR_ID: u16 = 0x1af4;
const VIRTIO_PCI_BLOCK_DEVICE_ID: u16 = 0x1042;

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    Virtio(virtio::Error),
    Partition(part::Error),
    Fat(fat::Error),
    Loader(loader::Error),
    Pe(pe::Error),
    ImageTooLarge,
}

fn boot_from_device(
    device: &mut block::VirtioBlockDevice,
    info: &dyn bootinfo::Info,
) -> Result<(), Error> {
    if let Err(err) = device.init() {
        error!("Error configuring block device: {:?}", err);
        return Err(Error::Virtio(err));
    }
    info!(
        "Virtio block device configured. Capacity: {} sectors",
        device.get_capacity()
    );

    let (start, end) = match part::find_efi_partition(device) {
        Ok(p) => p,
        Err(err) => {
            error!("Failed to find EFI partition: {:?}", err);
            return Err(Error::Partition(err));
        }
    };
    info!("Found EFI partition");

    let mut f = fat::Filesystem::new(device, start, end);
    if let Err(err) = f.init() {
        error!("Failed to create filesystem: {:?}", err);
        return Err(Error::Fat(err));
    }
    info!("Filesystem ready");

    match loader::load_default_entry(&f, info) {
        Ok(mut kernel) => {
            info!("Jumping to kernel");
            kernel.boot();
            return Ok(());
        }
        Err(err) => {
            warn!("Error loading default entry: {:?}", err);
            // Fall through to EFI boot
        }
    }

    info!("Using EFI boot.");

    let mut file = match f.open(efi::EFI_BOOT_PATH) {
        Ok(file) => file,
        Err(err) => {
            error!("Failed to load default EFI binary: {:?}", err);
            return Err(Error::Fat(err));
        }
    };
    info!("Found bootloader: {}", efi::EFI_BOOT_PATH);

    let mut l = pe::Loader::new(&mut file);

    let (entry_addr, load_addr, size) = match l.load(info.kernel_load_addr()) {
        Ok(load_info) => load_info,
        Err(err) => {
            error!("Error loading executable: {:?}", err);
            return Err(Error::Pe(err));
        }
    };

    #[cfg(target_arch = "aarch64")]
    if code_range().start < (info.kernel_load_addr() + size) as usize {
        error!("Error Boot Image is too large");
        return Err(Error::ImageTooLarge);
    }

    info!("Executable loaded");
    efi::efi_exec(entry_addr, load_addr, size, info, &f, device);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn rust64_start(#[cfg(not(feature = "coreboot"))] pvh_info: &pvh::StartInfo) -> ! {
    serial::PORT.borrow_mut().init();
    logger::init();

    arch::x86_64::sse::enable_sse();
    arch::x86_64::paging::setup();

    #[cfg(feature = "coreboot")]
    let info = &coreboot::StartInfo::default();

    #[cfg(not(feature = "coreboot"))]
    let info = pvh_info;

    main(info)
}

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn rust64_start(x0: *const u8) -> ! {
    arch::aarch64::simd::setup_simd();
    arch::aarch64::paging::setup();

    // Use atomic operation before MMU enabled may cause exception, see https://www.ipshop.xyz/5909.html
    serial::PORT.borrow_mut().init();
    logger::init();

    let info = fdt::StartInfo::new(
        x0,
        Some(arch::aarch64::layout::map::dram::ACPI_START as u64),
        arch::aarch64::layout::map::dram::KERNEL_START as u64,
        &crate::arch::aarch64::layout::MEM_LAYOUT[..],
        None,
    );

    if let Some((base, length)) = info.find_compatible_region(&["pci-host-ecam-generic"]) {
        pci::init(base as u64, length as u64);
    }

    main(&info)
}

#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub extern "C" fn rust64_start(a0: u64, a1: *const u8) -> ! {
    use crate::bootinfo::{EntryType, Info, MemoryEntry};

    serial::PORT.borrow_mut().init();
    logger::init();

    info!("Starting on RV64 0x{:x} 0x{:x}", a0, a1 as u64,);

    let info = fdt::StartInfo::new(
        a1,
        None,
        0x8040_0000,
        &crate::arch::riscv64::layout::MEM_LAYOUT[..],
        Some(MemoryEntry {
            addr: 0x4000_0000,
            size: 2 << 20,
            entry_type: EntryType::Reserved,
        }),
    );

    for i in 0..info.num_entries() {
        let region = info.entry(i);
        info!(
            "Memory region {}MiB@0x{:x}",
            region.size / 1024 / 1024,
            region.addr
        );
    }

    if let Some((base, length)) = info.find_compatible_region(&["pci-host-ecam-generic"]) {
        pci::init(base as u64, length as u64);
    }

    main(&info);
}

fn main(info: &dyn bootinfo::Info) -> ! {
    info!("Booting with {}", info.name());

    pci::print_bus();

    let mut next_address = info.pci_bar_memory().map(|m| m.addr);
    let max_address = info.pci_bar_memory().map(|m| m.addr + m.size);

    pci::with_devices(
        VIRTIO_PCI_VENDOR_ID,
        VIRTIO_PCI_BLOCK_DEVICE_ID,
        |mut pci_device| {
            pci_device.init();

            next_address = pci_device.allocate_bars(next_address);
            if next_address > max_address {
                panic!("PCI BAR allocation space exceeded")
            }

            let mut pci_transport = pci::VirtioPciTransport::new(pci_device);
            let mut device = block::VirtioBlockDevice::new(&mut pci_transport);
            boot_from_device(&mut device, info).is_ok()
        },
    );

    panic!("Unable to boot from any virtio-blk device")
}
