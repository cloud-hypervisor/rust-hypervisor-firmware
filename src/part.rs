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

use crate::block::SectorRead;

#[repr(packed)]
/// GPT header
struct Header {
    signature: u64,
    _revision: u32,
    _header_size: u32,
    _header_crc: u32,
    _reserved: u32,
    _current_lba: u64,
    _backup_lba: u64,
    first_usable_lba: u64,
    _last_usable_lba: u64,
    _disk_guid: [u8; 16],
    first_part_lba: u64,
    part_count: u32,
    _part_entry_size: u32,
    _part_crc: u32,
}

#[repr(packed)]
struct Partition {
    type_guid: [u8; 16],
    _guid: [u8; 16],
    first_lba: u64,
    last_lba: u64,
    _flags: u64,
    _partition_name: [u32; 18],
}

impl Partition {
    fn is_efi_partition(&self) -> bool {
        // GUID is C12A7328-F81F-11D2-BA4B-00A0C93EC93B in mixed-endian
        // 0-3, 4-5, 6-7 are LE, 8-19, and 10-15 are BE
        self.type_guid
            == [
                0x28, 0x73, 0x2a, 0xc1, // LE C12A7328
                0x1f, 0xf8, // LE F81F
                0xd2, 0x11, // LE 11D2
                0xba, 0x4b, // BE BA4B
                0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b, // BE 00A0C93EC93B
            ]
    }
}

pub enum Error {
    BlockError,
    HeaderNotFound,
    ViolatesSpecification,
    ExceededPartitionCount,
    NoEFIPartition,
}

/// Find EFI partition
pub fn find_efi_partition(r: &mut SectorRead) -> Result<(u64, u64), Error> {
    let mut data: [u8; 512] = [0; 512];
    match r.read(1, &mut data) {
        Ok(_) => {}
        Err(_) => return Err(Error::BlockError),
    };

    // Safe as sizeof header is less than 512 bytes (size of data)
    let h = unsafe { &*(data.as_ptr() as *const Header) };

    // GPT magic constant
    if h.signature != 0x5452415020494645u64 {
        return Err(Error::HeaderNotFound);
    }

    if h.first_usable_lba < 34 {
        return Err(Error::ViolatesSpecification);
    }

    let mut checked_part_count = 0u32;
    let part_count = h.part_count;
    let first_usable_lba = h.first_usable_lba;
    let first_part_lba = h.first_part_lba;

    for lba in first_part_lba..first_usable_lba {
        match r.read(lba, &mut data) {
            Ok(_) => {}
            Err(_) => return Err(Error::BlockError),
        }

        // Safe as size of partition struct * 4 is 512 bytes (size of data)
        let parts = unsafe { core::slice::from_raw_parts(data.as_ptr() as *const Partition, 4) };

        for p in parts {
            if p.is_efi_partition() {
                return Ok((p.first_lba, p.last_lba));
            }
            checked_part_count += 1;
            if checked_part_count == part_count {
                return Err(Error::ExceededPartitionCount);
            }
        }
    }

    Err(Error::NoEFIPartition)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::fs::File;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;

    use crate::block;
    use crate::block::SectorRead;

    struct FakeDisk {
        file: File,
    }

    impl FakeDisk {
        fn new() -> FakeDisk {
            let file =
                File::open("super_grub2_disk_x86_64_efi_2.02s10.iso").expect("missing disk image");
            return FakeDisk { file };
        }
    }

    impl SectorRead for FakeDisk {
        fn read(&mut self, sector: u64, data: &mut [u8]) -> Result<(), block::Error> {
            match self.file.seek(SeekFrom::Start(sector * 512)) {
                Ok(_) => {}
                Err(_) => return Err(block::Error::BlockIOError),
            }
            match self.file.read(data) {
                Ok(_) => {}
                Err(_) => return Err(block::Error::BlockIOError),
            }
            Ok(())
        }
    }

    #[test]
    fn test_find_efi_partition() {
        let mut d = FakeDisk::new();

        match super::find_efi_partition(&mut d) {
            Ok((start, end)) => {
                assert_eq!(start, 220);
                assert_eq!(end, 5979);
            }
            Err(e) => panic!(e),
        }
    }

}
