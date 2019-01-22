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

#[repr(C)]
/// GPT header
struct Header {
    signature: u64,
    revision: u32,
    header_size: u32,
    header_crc: u32,
    reserved: u32,
    current_lba: u64,
    backup_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    disk_guid: [u8; 16],
    first_part_lba: u64,
    part_count: u32,
    part_entry_size: u32,
    part_crc: u32,
}

pub enum Error {
    BlockError,
    HeaderNotFound,
}

/// Find partition table header
pub fn find_header(r: &mut SectorRead) -> Result<(), Error> {
    let mut data: [u8; 512] = [0; 512];
    match r.read(1, &mut data) {
        Ok(_) => {}
        Err(_) => return Err(Error::BlockError),
    };
    unsafe {
        let h = &*(data.as_ptr() as *const Header);

        // GPT magic constant
        if h.signature != 0x5452415020494645u64 {
            return Err(Error::HeaderNotFound);
        }
    }
    Ok(())
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
    fn test_find_part_header() {
        let mut d = FakeDisk::new();
        assert!(super::find_header(&mut d).is_ok(), "header should exist");
    }

}
