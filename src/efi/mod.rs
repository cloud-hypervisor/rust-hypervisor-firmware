// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use core::{cell::SyncUnsafeCell, ffi::c_void, mem::size_of, ptr::null_mut};

use atomic_refcell::AtomicRefCell;
use r_efi::{
    efi::{self, Guid, Handle, Status},
    protocols::loaded_image::Protocol as LoadedImageProtocol,
};

use crate::{bootinfo, layout};

mod alloc;
mod block;
mod boot_services;
mod console;
mod device_path;
mod file;
mod mem_file;
mod runtime_services;
mod var;

use alloc::Allocator;
use boot_services::{BS, CT};
use device_path::DevicePath;
use runtime_services::RS;
use var::VariableAllocator;

#[cfg(target_arch = "aarch64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTAA64.EFI";
#[cfg(target_arch = "x86_64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTX64.EFI";
#[cfg(target_arch = "riscv64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTRISCV64.EFI";

#[derive(Copy, Clone, PartialEq, Eq)]
enum HandleType {
    None,
    Block,
    FileSystem,
    LoadedImage,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct HandleWrapper {
    handle_type: HandleType,
}

pub static ALLOCATOR: AtomicRefCell<Allocator> = AtomicRefCell::new(Allocator::new());

pub static VARIABLES: AtomicRefCell<VariableAllocator> =
    AtomicRefCell::new(VariableAllocator::new());

// RHF string in UCS-2
const FIRMWARE_STRING: [u16; 4] = [0x0052, 0x0048, 0x0046, 0x0000];

static mut ST: SyncUnsafeCell<efi::SystemTable> = SyncUnsafeCell::new(efi::SystemTable {
    hdr: efi::TableHeader {
        signature: efi::SYSTEM_TABLE_SIGNATURE,
        revision: (2 << 16) | (80),
        header_size: size_of::<efi::SystemTable>() as u32,
        crc32: 0, // TODO
        reserved: 0,
    },
    firmware_vendor: FIRMWARE_STRING.as_ptr() as *mut u16,
    firmware_revision: 0,
    console_in_handle: console::STDIN_HANDLE,
    con_in: null_mut(),
    console_out_handle: console::STDOUT_HANDLE,
    con_out: null_mut(),
    standard_error_handle: console::STDERR_HANDLE,
    std_err: null_mut(),
    runtime_services: null_mut(),
    boot_services: null_mut(),
    number_of_table_entries: 0,
    configuration_table: null_mut(),
});

static mut BLOCK_WRAPPERS: SyncUnsafeCell<block::BlockWrappers> =
    SyncUnsafeCell::new(block::BlockWrappers {
        wrappers: [null_mut(); 16],
        count: 0,
    });

const PAGE_SIZE: u64 = 4096;

// Populate allocator from E820, fixed ranges for the firmware and the loaded binary.
fn populate_allocator(info: &dyn bootinfo::Info, image_address: u64, image_size: u64) {
    for i in 0..info.num_entries() {
        let entry = info.entry(i);
        match entry.entry_type {
            bootinfo::EntryType::Ram => {
                ALLOCATOR.borrow_mut().add_initial_allocation(
                    efi::CONVENTIONAL_MEMORY,
                    entry.size / PAGE_SIZE,
                    entry.addr,
                    efi::MEMORY_WB,
                );
            }
            _ => continue,
        }
    }

    for descriptor in info.memory_layout() {
        let memory_type = match descriptor.attribute {
            layout::MemoryAttribute::Code => efi::RUNTIME_SERVICES_CODE,
            layout::MemoryAttribute::Data => efi::RUNTIME_SERVICES_DATA,
            layout::MemoryAttribute::Unusable => efi::UNUSABLE_MEMORY,
            layout::MemoryAttribute::Mmio => efi::MEMORY_MAPPED_IO,
        };
        ALLOCATOR.borrow_mut().allocate_pages(
            efi::ALLOCATE_ADDRESS,
            memory_type,
            descriptor.page_count() as u64,
            descriptor.range_start() as u64,
        );
    }

    if let Some(fdt_entry) = info.fdt_reservation() {
        ALLOCATOR.borrow_mut().allocate_pages(
            efi::ALLOCATE_ADDRESS,
            efi::UNUSABLE_MEMORY,
            (fdt_entry.size + 4095) / 4096,
            fdt_entry.addr,
        );
    }

    // Add the loaded binary
    ALLOCATOR.borrow_mut().allocate_pages(
        efi::ALLOCATE_ADDRESS,
        efi::LOADER_CODE,
        image_size / PAGE_SIZE,
        image_address,
    );
}

#[repr(C)]
struct LoadedImageWrapper {
    hw: HandleWrapper,
    proto: LoadedImageProtocol,
    entry_point: u64,
}

fn new_image_handle(
    file_path: *mut r_efi::protocols::device_path::Protocol,
    parent_handle: Handle,
    device_handle: Handle,
    load_addr: u64,
    load_size: u64,
    entry_addr: u64,
) -> *mut LoadedImageWrapper {
    let mut image = null_mut();
    let status = boot_services::allocate_pool(
        efi::LOADER_DATA,
        size_of::<LoadedImageWrapper>(),
        &mut image as *mut *mut c_void,
    );
    assert!(status == Status::SUCCESS);
    let image = unsafe { &mut *(image as *mut LoadedImageWrapper) };
    *image = LoadedImageWrapper {
        hw: HandleWrapper {
            handle_type: HandleType::LoadedImage,
        },
        proto: LoadedImageProtocol {
            revision: r_efi::protocols::loaded_image::REVISION,
            parent_handle,
            system_table: unsafe { ST.get_mut() },
            device_handle,
            file_path,
            load_options_size: 0,
            load_options: null_mut(),
            image_base: load_addr as *mut _,
            image_size: load_size,
            image_code_type: efi::LOADER_CODE,
            image_data_type: efi::LOADER_DATA,
            unload: boot_services::unload_image,
            reserved: null_mut(),
        },
        entry_point: entry_addr,
    };
    image
}

pub fn efi_exec(
    address: u64,
    loaded_address: u64,
    loaded_size: u64,
    info: &dyn bootinfo::Info,
    fs: &crate::fat::Filesystem,
    block: &crate::block::VirtioBlockDevice,
) {
    let vendor_data = 0u32;

    let ct = unsafe { CT.get_mut() };
    let mut ct_index = 0;

    // Populate with FDT table if present
    // To ensure ACPI is used during boot do not include FDT table on aarch64
    // https://github.com/torvalds/linux/blob/d528014517f2b0531862c02865b9d4c908019dc4/arch/arm64/kernel/acpi.c#L203
    #[cfg(not(target_arch = "aarch64"))]
    if let Some(fdt_entry) = info.fdt_reservation() {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0xb1b621d5,
                0xf19c,
                0x41a5,
                0x83,
                0x0b,
                &[0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0],
            ),
            vendor_table: fdt_entry.addr as *const u64 as *mut _,
        };
        ct_index += 1;
    }

    // Populate with ACPI RSDP table if present
    if let Some(acpi_rsdp_ptr) = info.rsdp_addr() {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0x8868_e871,
                0xe4f1,
                0x11d3,
                0xbc,
                0x22,
                &[0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
            ),
            vendor_table: acpi_rsdp_ptr as *mut _,
        };
        ct_index += 1;
    }

    // Othwerwise fill with zero vendor data
    if ct_index == 0 {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0x678a_9665,
                0x9957,
                0x4e7c,
                0xa6,
                0x27,
                &[0x34, 0xc9, 0x46, 0x3d, 0xd2, 0xac],
            ),
            vendor_table: &vendor_data as *const _ as *mut _,
        }
    };

    let mut stdin = console::STDIN;
    let mut stdout = console::STDOUT;
    let st = unsafe { ST.get_mut() };
    st.con_in = &mut stdin;
    st.con_out = &mut stdout;
    st.std_err = &mut stdout;
    st.runtime_services = unsafe { RS.get_mut() };
    st.boot_services = unsafe { BS.get_mut() };
    st.number_of_table_entries = 1;
    st.configuration_table = &mut ct[0];

    populate_allocator(info, loaded_address, loaded_size);

    let efi_part_id = unsafe { block::populate_block_wrappers(BLOCK_WRAPPERS.get_mut(), block) };

    let wrapped_fs = file::FileSystemWrapper::new(fs, efi_part_id);

    let mut path = [0u8; 256];
    path[0..crate::efi::EFI_BOOT_PATH.as_bytes().len()]
        .copy_from_slice(crate::efi::EFI_BOOT_PATH.as_bytes());
    let device_path = DevicePath::File(path);
    let image = new_image_handle(
        device_path.generate(),
        0 as Handle,
        &wrapped_fs as *const _ as Handle,
        loaded_address,
        loaded_size,
        address,
    );

    let ptr = address as *const ();
    let code: extern "efiapi" fn(Handle, *mut efi::SystemTable) -> Status =
        unsafe { core::mem::transmute(ptr) };
    (code)((image as *const _) as Handle, &mut *st);
}
