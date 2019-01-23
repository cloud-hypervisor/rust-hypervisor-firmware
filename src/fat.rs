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

#[derive(Debug)]
#[repr(packed)]
struct Header {
    magic: [u8; 3],
    identifier: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    directory_count: u16,
    legacy_sectors: u16,
    media_type: u8,
    legacy_sectors_per_fat: u16,
    sectors_per_track: u16,
    head_count: u16,
    hidden_sectors: u32,
    sectors: u32,
}

#[derive(Debug)]
#[repr(packed)]
struct Fat12Header {
    header: Header,
    drive_number: u8,
    signature: u8,
    nt_flags: u8,
    serial: u32,
    volume: [u8; 11],
    id: [u8; 8],
}

#[derive(Debug)]
#[repr(packed)]
struct Fat32Header {
    header: Header,
    sectors_per_fat: u32,
    flags: u16,
    version: u16,
    root_cluster: u32,
    fsinfo_sector: u16,
    backup_boot_sector: u16,
    reserved: [u8; 12],
    drive_no: u8,
    nt_flags: u8,
    signature: u8,
    serial: u32,
    volume: [u8; 11],
    id: [u8; 8],
}

enum FatType {
    Unknown,
    FAT12,
    FAT16,
    FAT32,
}

pub struct Filesystem<'a> {
    device: &'a mut SectorRead,
    start: u64,
    sectors: u32,
    fat_type: FatType,
}

pub enum Error {
    BlockError,
}

impl<'a> Filesystem<'a> {
    pub fn new(device: &'a mut SectorRead, start: u64) -> Filesystem {
        Filesystem {
            device,
            start,
            sectors: 0,
            fat_type: FatType::Unknown,
        }
    }

    pub fn init(&mut self) -> Result<(), Error> {
        const FAT12_MAX: u32 = 0xff5;
        const FAT16_MAX: u32 = 0xfff5;

        let mut data: [u8; 512] = [0; 512];
        match self.device.read(self.start, &mut data) {
            Ok(_) => {}
            Err(_) => return Err(Error::BlockError),
        };

        let h = unsafe { &*(data.as_ptr() as *const Header) };

        self.sectors = if h.legacy_sectors == 0 {
            h.sectors
        } else {
            h.legacy_sectors as u32
        };

        self.fat_type = if self.sectors < FAT12_MAX {
            FatType::FAT12
        } else if self.sectors < FAT16_MAX {
            FatType::FAT16
        } else {
            FatType::FAT32
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::part::tests::FakeDisk;
    #[test]
    fn test_fat_init() {
        let mut d = FakeDisk::new();
        match crate::part::find_efi_partition(&mut d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&mut d, start);
                match f.init() {
                    Ok(()) => {
                        assert_eq!(f.sectors, 5760);
                        assert_eq!(f.fat_type, super::FatType::FAT12);
                    }
                    Err(e) => panic!(e),
                }
            }
            Err(e) => panic!(e),
        }
    }
}
