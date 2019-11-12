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

use r_efi::efi::{AllocateType, Guid, MemoryType, Status};
use r_efi::protocols::device_path::Protocol as DevicePathProtocol;
use r_efi::{eficall, eficall_abi};

pub const PROTOCOL_GUID: Guid = Guid::from_fields(
    0x964e_5b21,
    0x6459,
    0x11d2,
    0x8e,
    0x39,
    &[0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
);

#[repr(packed)]
pub struct HardDiskDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub partition_number: u32,
    pub partition_start: u64,
    pub partition_size: u64,
    pub partition_signature: [u8; 16],
    pub partition_format: u8,
    pub signature_type: u8,
}

#[repr(packed)]
pub struct ControllerDevicePathProtocol {
    pub device_path: DevicePathProtocol,
    pub controller: u32,
}

#[repr(C)]
struct BlockIoMedia {
    media_id: u32,
    removable_media: bool,
    media_present: bool,
    logical_partition: bool,
    read_only: bool,
    write_caching: bool,
    block_size: u32,
    io_align: u32,
    last_block: u64,
}

#[repr(C)]
pub struct BlockIoProtocol {
    revision: u64,
    media: *const BlockIoMedia,
    reset: eficall! {fn(
        *mut BlockIoProtocol,
        bool
    ) -> Status},
    read_blocks: eficall! {fn(
        *mut BlockIoProtocol,
        u32,
        u64,
        usize,
        *mut c_void
    ) -> Status},
    write_blocks: eficall! {fn(
        *mut BlockIoProtocol,
        u32,
        u64,
        usize,
        *mut c_void
    ) -> Status},
    flush_blocks: eficall! {fn(
        *mut BlockIoProtocol,
    ) -> Status},
}

#[repr(C)]
pub struct BlockWrapper<'a> {
    hw: super::HandleWrapper,
    block: *const crate::block::VirtioBlockDevice<'a>,
    media: BlockIoMedia,
    pub proto: BlockIoProtocol,
    // The ordering of these paths are very important, along with the C
    // representation as the device path "flows" from the first.
    pub controller_path: ControllerDevicePathProtocol,
    pub disk_paths: [HardDiskDevicePathProtocol; 2],
    start_lba: u64,
}

pub struct BlockWrappers<'a> {
    pub wrappers: [*mut BlockWrapper<'a>; 16],
    pub count: usize,
}

pub extern "win64" fn reset(_: *mut BlockIoProtocol, _: bool) -> Status {
    Status::UNSUPPORTED
}

pub extern "win64" fn read_blocks(
    proto: *mut BlockIoProtocol,
    _: u32,
    start: u64,
    size: usize,
    buffer: *mut c_void,
) -> Status {
    let wrapper = container_of!(proto, BlockWrapper, proto);
    let wrapper = unsafe { &*wrapper };

    let blocks = (size / 512) as usize;
    let mut region = crate::mem::MemoryRegion::new(buffer as u64, size as u64);

    for i in 0..blocks {
        use crate::block::SectorRead;
        let data = region.as_mut_slice(i as u64 * 512, 512);
        let block = unsafe { &*wrapper.block };
        match block.read(wrapper.start_lba + start + i as u64, data) {
            Ok(()) => continue,
            Err(_) => {
                return Status::DEVICE_ERROR;
            }
        };
    }

    Status::SUCCESS
}

pub extern "win64" fn write_blocks(
    proto: *mut BlockIoProtocol,
    _: u32,
    start: u64,
    size: usize,
    buffer: *mut c_void,
) -> Status {
    let wrapper = container_of!(proto, BlockWrapper, proto);
    let wrapper = unsafe { &*wrapper };

    let blocks = (size / 512) as usize;
    let mut region = crate::mem::MemoryRegion::new(buffer as u64, size as u64);

    for i in 0..blocks {
        use crate::block::SectorWrite;
        let data = region.as_mut_slice(i as u64 * 512, 512);
        let block = unsafe { &*wrapper.block };
        match block.write(wrapper.start_lba + start + i as u64, data) {
            Ok(()) => continue,
            Err(_) => {
                return Status::DEVICE_ERROR;
            }
        };
    }

    Status::SUCCESS
}

pub extern "win64" fn flush_blocks(proto: *mut BlockIoProtocol) -> Status {
    let wrapper = container_of!(proto, BlockWrapper, proto);
    let wrapper = unsafe { &*wrapper };
    use crate::block::SectorWrite;
    let block = unsafe { &*wrapper.block };
    match block.flush() {
        Ok(()) => Status::SUCCESS,
        Err(_) => Status::DEVICE_ERROR,
    }
}

impl<'a> BlockWrapper<'a> {
    pub fn new(
        block: *const crate::block::VirtioBlockDevice,
        partition_number: u32,
        start_lba: u64,
        last_lba: u64,
        uuid: [u8; 16],
    ) -> *mut BlockWrapper {
        let last_block = unsafe { (*block).get_capacity() } - 1;

        let size = core::mem::size_of::<BlockWrapper>();
        let (_status, new_address) = super::ALLOCATOR.lock().allocate_pages(
            AllocateType::AllocateAnyPages,
            MemoryType::LoaderData,
            ((size + super::PAGE_SIZE as usize - 1) / super::PAGE_SIZE as usize) as u64,
            0 as u64,
        );

        let bw = new_address as *mut BlockWrapper;

        unsafe {
            *bw = BlockWrapper {
                hw: super::HandleWrapper {
                    handle_type: super::HandleType::Block,
                },
                block,
                media: BlockIoMedia {
                    media_id: 0,
                    removable_media: false,
                    media_present: true,
                    logical_partition: false,
                    read_only: true,
                    write_caching: false,
                    block_size: 512,
                    io_align: 0,
                    last_block,
                },
                proto: BlockIoProtocol {
                    revision: 0x0001_0000, // EFI_BLOCK_IO_PROTOCOL_REVISION
                    media: core::ptr::null(),
                    reset,
                    read_blocks,
                    write_blocks,
                    flush_blocks,
                },
                start_lba,
                controller_path: ControllerDevicePathProtocol {
                    device_path: DevicePathProtocol {
                        r#type: 1,
                        sub_type: 5,
                        length: [8, 0],
                    },
                    controller: 0,
                },
                // full disk vs partition
                disk_paths: if partition_number == 0 {
                    [
                        HardDiskDevicePathProtocol {
                            device_path: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_END,
                                sub_type: 0xff, // End of full path
                                length: [4, 0],
                            },
                            partition_number: 0,
                            partition_format: 0x0,
                            partition_start: 0,
                            partition_size: 0,
                            partition_signature: [0; 16],
                            signature_type: 0,
                        },
                        HardDiskDevicePathProtocol {
                            device_path: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_END,
                                sub_type: 0xff, // End of full path
                                length: [4, 0],
                            },
                            partition_number: 0,
                            partition_format: 0x0,
                            partition_start: 0,
                            partition_size: 0,
                            partition_signature: [0; 16],
                            signature_type: 0,
                        },
                    ]
                } else {
                    [
                        HardDiskDevicePathProtocol {
                            device_path: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_MEDIA,
                                sub_type: 1,
                                length: [42, 0],
                            },
                            partition_number,
                            partition_format: 0x02, // GPT
                            partition_start: start_lba,
                            partition_size: last_lba - start_lba + 1,
                            partition_signature: uuid,
                            signature_type: 0x02,
                        },
                        HardDiskDevicePathProtocol {
                            device_path: DevicePathProtocol {
                                r#type: r_efi::protocols::device_path::TYPE_END,
                                sub_type: 0xff, // End of full path
                                length: [4, 0],
                            },
                            partition_number: 0,
                            partition_format: 0x0,
                            partition_start: 0,
                            partition_size: 0,
                            partition_signature: [0; 16],
                            signature_type: 0,
                        },
                    ]
                },
            };

            (*bw).proto.media = &(*bw).media;
        }
        bw
    }
}

#[allow(clippy::transmute_ptr_to_ptr)]
pub fn populate_block_wrappers(
    wrappers: &mut BlockWrappers,
    block: *const crate::block::VirtioBlockDevice,
) -> Option<u32> {
    let mut parts: [crate::part::PartitionEntry; 16] = unsafe { core::mem::zeroed() };

    wrappers.wrappers[0] =
        BlockWrapper::new(unsafe { core::mem::transmute(block) }, 0, 0, 0, [0; 16]);

    let mut efi_part_id = None;
    let part_count = crate::part::get_partitions(unsafe { &*block }, &mut parts).unwrap();
    for i in 0..part_count {
        let p = parts[i as usize];
        wrappers.wrappers[i as usize + 1] = BlockWrapper::new(
            unsafe { core::mem::transmute(block) },
            i + 1,
            p.first_lba,
            p.last_lba,
            p.guid,
        );
        if p.is_efi_partition() {
            efi_part_id = Some(i + 1);
        }
    }
    wrappers.count = part_count as usize + 1;
    efi_part_id
}
