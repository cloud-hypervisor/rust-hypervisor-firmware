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

use core::ffi::c_void;

use r_efi::{
    efi::{AllocateType, Char16, Guid, MemoryType, Status},
    protocols::{
        device_path::Protocol as DevicePathProtocol, file::Protocol as FileProtocol,
        simple_file_system::Protocol as SimpleFileSystemProtocol,
    },
};

#[repr(C)]
pub struct FileDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub filename: [u16; 64],
}

pub extern "win64" fn filesystem_open_volume(
    fs_proto: *mut SimpleFileSystemProtocol,
    file: *mut *mut FileProtocol,
) -> Status {
    let wrapper = container_of!(fs_proto, FileSystemWrapper, proto);
    let wrapper = unsafe { &*wrapper };

    if let Some(fw) = wrapper.create_file(true) {
        unsafe {
            *file = &mut (*fw).proto;
        }
        Status::SUCCESS
    } else {
        Status::DEVICE_ERROR
    }
}

pub extern "win64" fn open(
    file_in: *mut FileProtocol,
    file_out: *mut *mut FileProtocol,
    path_in: *mut Char16,
    _: u64,
    _: u64,
) -> Status {
    let wrapper = container_of!(file_in, FileWrapper, proto);
    let wrapper = unsafe { &*wrapper };

    if !wrapper.root {
        log!("Attempt to open file from non-root file is unsupported");
        return Status::UNSUPPORTED;
    }

    let mut path = [0; 256];
    crate::common::ucs2_to_ascii(path_in, &mut path[0..255]);
    let path = unsafe { core::str::from_utf8_unchecked(&path) };

    match wrapper.fs.open(path) {
        Ok(f) => {
            let fs_wrapper = unsafe { &(*wrapper.fs_wrapper) };
            if let Some(file_out_wrapper) = fs_wrapper.create_file(false) {
                unsafe {
                    (*file_out_wrapper).file = f;
                    *file_out = &mut (*file_out_wrapper).proto;
                }
                Status::SUCCESS
            } else {
                Status::DEVICE_ERROR
            }
        }
        Err(_) => Status::DEVICE_ERROR,
    }
}

pub extern "win64" fn close(proto: *mut FileProtocol) -> Status {
    let wrapper = container_of!(proto, FileWrapper, proto);
    super::ALLOCATOR
        .lock()
        .free_pages(&wrapper as *const _ as u64)
}

pub extern "win64" fn delete(_: *mut FileProtocol) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn read(file: *mut FileProtocol, size: *mut usize, buf: *mut c_void) -> Status {
    let wrapper = container_of_mut!(file, FileWrapper, proto);

    let mut current_offset = 0;
    let mut bytes_remaining = unsafe { *size };

    loop {
        use crate::fat::Read;
        let buf = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, *size) };

        let mut data: [u8; 512] = [0; 512];
        unsafe {
            match (*wrapper).file.read(&mut data) {
                Ok(bytes_read) => {
                    buf[current_offset..current_offset + bytes_read as usize]
                        .copy_from_slice(&data[0..bytes_read as usize]);
                    current_offset += bytes_read as usize;

                    if bytes_remaining <= bytes_read as usize {
                        *size = current_offset;
                        return Status::SUCCESS;
                    }
                    bytes_remaining -= bytes_read as usize;
                }
                Err(_) => {
                    return Status::DEVICE_ERROR;
                }
            }
        }
    }
}

pub extern "win64" fn write(_: *mut FileProtocol, _: *mut usize, _: *mut c_void) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn get_position(_: *mut FileProtocol, _: *mut u64) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn set_position(_: *mut FileProtocol, _: u64) -> Status {
    Status::UNSUPPORTED
}

struct FileInfo {
    size: u64,
    file_size: u64,
    physical_size: u64,
    _create_time: r_efi::system::Time,
    _last_access_time: r_efi::system::Time,
    _modification_time: r_efi::system::Time,
    attribute: u64,
    _file_name: [Char16; 256],
}

pub extern "win64" fn get_info(
    file: *mut FileProtocol,
    guid: *mut Guid,
    info_size: *mut usize,
    info: *mut c_void,
) -> Status {
    if unsafe { *guid } == r_efi::protocols::file::INFO_ID {
        if unsafe { *info_size } < core::mem::size_of::<FileInfo>() {
            unsafe { *info_size = core::mem::size_of::<FileInfo>() };
            Status::BUFFER_TOO_SMALL
        } else {
            let info = info as *mut FileInfo;

            let wrapper = container_of!(file, FileWrapper, proto);
            use crate::fat::Read;
            unsafe {
                (*info).size = core::mem::size_of::<FileInfo>() as u64;
                (*info).file_size = (*wrapper).file.get_size().into();
                (*info).physical_size = (*wrapper).file.get_size().into();
                (*info).attribute = r_efi::protocols::file::MODE_READ;
            }

            Status::SUCCESS
        }
    } else {
        Status::UNSUPPORTED
    }
}

pub extern "win64" fn set_info(
    _: *mut FileProtocol,
    _: *mut Guid,
    _: usize,
    _: *mut c_void,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn flush(_: *mut FileProtocol) -> Status {
    Status::UNSUPPORTED
}

struct FileWrapper<'a> {
    fs: &'a crate::fat::Filesystem<'a>,
    proto: FileProtocol,
    file: crate::fat::File<'a>,
    fs_wrapper: *const FileSystemWrapper<'a>,
    root: bool,
}

#[repr(C)]
pub struct FileSystemWrapper<'a> {
    hw: super::HandleWrapper,
    fs: &'a crate::fat::Filesystem<'a>,
    pub proto: SimpleFileSystemProtocol,
    pub block_part_id: Option<u32>,
}

impl<'a> FileSystemWrapper<'a> {
    fn create_file(&self, root: bool) -> Option<*mut FileWrapper> {
        let size = core::mem::size_of::<FileWrapper>();
        let (status, new_address) = super::ALLOCATOR.lock().allocate_pages(
            AllocateType::AllocateAnyPages,
            MemoryType::LoaderData,
            ((size + super::PAGE_SIZE as usize - 1) / super::PAGE_SIZE as usize) as u64,
            0 as u64,
        );

        if status == Status::SUCCESS {
            let fw = new_address as *mut FileWrapper;
            unsafe {
                (*fw).fs = self.fs;
                (*fw).fs_wrapper = self;
                (*fw).root = root;
                (*fw).proto.revision = r_efi::protocols::file::REVISION;
                (*fw).proto.open = open;
                (*fw).proto.close = close;
                (*fw).proto.delete = delete;
                (*fw).proto.read = read;
                (*fw).proto.write = write;
                (*fw).proto.get_position = get_position;
                (*fw).proto.set_position = set_position;
                (*fw).proto.get_info = get_info;
                (*fw).proto.set_info = set_info;
                (*fw).proto.flush = flush;
            }

            Some(fw)
        } else {
            None
        }
    }

    pub fn new(
        fs: &'a crate::fat::Filesystem,
        block_part_id: Option<u32>,
    ) -> FileSystemWrapper<'a> {
        FileSystemWrapper {
            hw: super::HandleWrapper {
                handle_type: super::HandleType::FileSystem,
            },
            fs,
            proto: SimpleFileSystemProtocol {
                revision: r_efi::protocols::simple_file_system::REVISION,
                open_volume: filesystem_open_volume,
            },
            block_part_id,
        }
    }
}
