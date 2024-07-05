// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Rivos Inc.

use core::{
    ffi::c_void,
    mem::{offset_of, size_of},
    ptr::null_mut,
};

use log::error;
use r_efi::{
    efi::{self, MemoryType, Status},
    protocols::device_path::Protocol as DevicePathProtocol,
};

#[allow(clippy::large_enum_variant)]
pub enum DevicePath {
    File([u8; 256]),
    Memory(MemoryType, u64, u64),
    Unsupported,
}

impl DevicePath {
    pub fn parse(dpp: &DevicePathProtocol) -> DevicePath {
        let mut dpp = dpp;
        loop {
            if dpp.r#type == r_efi::protocols::device_path::TYPE_MEDIA && dpp.sub_type == 0x04 {
                let ptr = (dpp as *const _ as usize + offset_of!(FileDevicePathProtocol, filename))
                    as *const u16;
                let mut path = [0u8; 256];
                crate::common::ucs2_to_ascii(ptr, &mut path);
                return DevicePath::File(path);
            }
            if dpp.r#type == r_efi::protocols::device_path::TYPE_HARDWARE
                && dpp.sub_type == r_efi::protocols::device_path::Hardware::SUBTYPE_MMAP
            {
                let memory_type_ptr = (dpp as *const _ as usize
                    + offset_of!(MemoryDevicePathProtocol, memory_type))
                    as *const MemoryType;
                let start_ptr = (dpp as *const _ as usize
                    + offset_of!(MemoryDevicePathProtocol, start))
                    as *const u64;
                let end_ptr = (dpp as *const _ as usize + offset_of!(MemoryDevicePathProtocol, end))
                    as *const u64;
                return DevicePath::Memory(
                    unsafe { *memory_type_ptr },
                    unsafe { *start_ptr },
                    unsafe { *end_ptr },
                );
            }

            if dpp.r#type == r_efi::protocols::device_path::TYPE_END && dpp.sub_type == 0xff {
                error!("Unexpected end of device path");
                return DevicePath::Unsupported;
            }
            let len = unsafe { core::mem::transmute::<[u8; 2], u16>(dpp.length) };
            dpp = unsafe { &*((dpp as *const _ as u64 + len as u64) as *const _) };
        }
    }

    pub fn generate(&self) -> *mut r_efi::protocols::device_path::Protocol {
        match self {
            Self::File(path) => file_device_path(crate::common::ascii_strip(path)),
            Self::Memory(memory_type, start, end) => memory_device_path(*memory_type, *start, *end),
            Self::Unsupported => panic!("Cannot generate from unsupported Device Path type"),
        }
    }
}

#[repr(C)]
struct FileDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub filename: [u16; 256],
}

type FileDevicePaths = [FileDevicePathProtocol; 2];

fn file_device_path(path: &str) -> *mut r_efi::protocols::device_path::Protocol {
    let mut file_paths = null_mut();
    let status = crate::efi::boot_services::allocate_pool(
        efi::LOADER_DATA,
        size_of::<FileDevicePaths>(),
        &mut file_paths as *mut *mut c_void,
    );
    assert!(status == Status::SUCCESS);
    let file_paths = unsafe { &mut *(file_paths as *mut FileDevicePaths) };
    *file_paths = [
        FileDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_MEDIA,
                sub_type: 4, // Media Path type file
                length: (size_of::<FileDevicePathProtocol>() as u16).to_le_bytes(),
            },
            filename: [0; 256],
        },
        FileDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_END,
                sub_type: r_efi::protocols::device_path::End::SUBTYPE_ENTIRE,
                length: (size_of::<DevicePathProtocol>() as u16).to_le_bytes(),
            },
            filename: [0; 256],
        },
    ];

    crate::common::ascii_to_ucs2(path, &mut file_paths[0].filename);

    &mut file_paths[0].device_path // Pointer to first path entry
}

#[repr(C)]
struct MemoryDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub memory_type: u32,
    pub start: u64,
    pub end: u64,
}

type MemoryDevicePaths = [MemoryDevicePathProtocol; 2];

fn memory_device_path(
    memory_type: MemoryType,
    start: u64,
    end: u64,
) -> *mut r_efi::protocols::device_path::Protocol {
    let mut memory_paths = null_mut();
    let status = crate::efi::boot_services::allocate_pool(
        efi::LOADER_DATA,
        size_of::<MemoryDevicePaths>(),
        &mut memory_paths as *mut *mut c_void,
    );
    assert!(status == Status::SUCCESS);
    let memory_paths = unsafe { &mut *(memory_paths as *mut MemoryDevicePaths) };
    *memory_paths = [
        MemoryDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_HARDWARE,
                sub_type: r_efi::protocols::device_path::Hardware::SUBTYPE_MMAP,
                length: (size_of::<MemoryDevicePathProtocol>() as u16).to_le_bytes(),
            },
            memory_type,
            start,
            end,
        },
        MemoryDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_END,
                sub_type: r_efi::protocols::device_path::End::SUBTYPE_ENTIRE,
                length: (size_of::<DevicePathProtocol>() as u16).to_le_bytes(),
            },
            memory_type: 0,
            start: 0,
            end: 0,
        },
    ];

    &mut memory_paths[0].device_path // Pointer to first path entry
}
