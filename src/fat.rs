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
    root_dir_count: u16,
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

#[repr(packed)]
#[derive(Debug)]
struct Directory {
    name: [u8; 11],
    flags: u8,
    unused1: [u8; 8],
    cluster_high: u16,
    unused2: [u8; 4],
    cluster_low: u16,
    size: u32,
}

#[derive(Debug, PartialEq)]
enum FatType {
    Unknown,
    FAT12,
    FAT16,
    FAT32,
}

pub struct Filesystem<'a> {
    device: &'a mut SectorRead,
    start: u64,
    bytes_per_sector: u32,
    sectors: u32,
    fat_type: FatType,
    clusters: u32,
    sectors_per_fat: u32,
    sectors_per_cluster: u32,
    fat_count: u32,
    root_dir_sectors: u32,
    first_fat_sector: u32,
    first_data_sector: u32,
    data_sector_count: u32,
    data_cluster_count: u32,
    root_cluster: u32, // FAT32 only
}

#[derive(Debug)]
pub enum Error {
    BlockError,
    Unsupported,
    NotFound,
    EndOfFile,
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    File,
    Directory,
}

impl<'a> SectorRead for Filesystem<'a> {
    fn read(&mut self, sector: u64, data: &mut [u8]) -> Result<(), crate::block::Error> {
        self.device.read(self.start + sector, data)
    }
}

impl<'a> Filesystem<'a> {
    pub fn new(device: &'a mut SectorRead, start: u64) -> Filesystem {
        Filesystem {
            device,
            start,
            bytes_per_sector: 0,
            sectors: 0,
            fat_type: FatType::Unknown,
            clusters: 0,
            sectors_per_fat: 0,
            sectors_per_cluster: 0,
            fat_count: 0,
            root_dir_sectors: 0,
            first_fat_sector: 0,
            first_data_sector: 0,
            data_sector_count: 0,
            data_cluster_count: 0,
            root_cluster: 0,
        }
    }

    pub fn init(&mut self) -> Result<(), Error> {
        const FAT12_MAX: u32 = 0xff5;
        const FAT16_MAX: u32 = 0xfff5;

        let mut data: [u8; 512] = [0; 512];
        match self.read(0, &mut data) {
            Ok(_) => {}
            Err(_) => return Err(Error::BlockError),
        };

        let h = unsafe { &*(data.as_ptr() as *const Header) };

        self.bytes_per_sector = h.bytes_per_sector as u32;
        self.fat_count = h.fat_count as u32;
        self.sectors_per_cluster = h.sectors_per_cluster as u32;

        self.sectors = if h.legacy_sectors == 0 {
            h.sectors
        } else {
            h.legacy_sectors as u32
        };

        self.clusters = self.sectors / h.sectors_per_cluster as u32;

        self.fat_type = if self.clusters < FAT12_MAX {
            FatType::FAT12
        } else if self.clusters < FAT16_MAX {
            FatType::FAT16
        } else {
            FatType::FAT32
        };

        if self.fat_type == FatType::FAT32 {
            let h32 = unsafe { &*(data.as_ptr() as *const Fat32Header) };
            self.sectors_per_fat = h32.sectors_per_fat;
            self.root_cluster = h32.root_cluster;
        } else {
            self.sectors_per_fat = h.legacy_sectors_per_fat as u32;
        }

        if self.fat_type == FatType::FAT12 || self.fat_type == FatType::FAT16 {
            self.root_dir_sectors = ((h.root_dir_count as u32 * 32) + self.bytes_per_sector - 1)
                / self.bytes_per_sector;
        }

        self.first_fat_sector = h.reserved_sectors as u32;
        self.first_data_sector =
            self.first_fat_sector + (self.fat_count * self.sectors_per_fat) + self.root_dir_sectors;
        self.data_sector_count = self.sectors - self.first_data_sector;
        self.data_cluster_count = self.data_sector_count / self.bytes_per_sector;

        Ok(())
    }

    fn next_cluster(&mut self, cluster: u32) -> Result<u32, Error> {
        match self.fat_type {
            FatType::FAT12 => {
                let mut data: [u8; 512] = [0; 512];

                let fat_offset = cluster + (cluster / 2); // equivalent of x 1.5
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                match self.read(fat_sector as u64, &mut data) {
                    Ok(_) => {}
                    Err(_) => return Err(Error::BlockError),
                };

                let next_cluster_raw =
                    unsafe { *((data.as_ptr() as u64 + offset as u64) as *const u16) };

                let next_cluster = if cluster % 2 == 0 {
                    next_cluster_raw & 0xfff
                } else {
                    next_cluster_raw >> 4
                };
                if next_cluster >= 0xff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(next_cluster as u32)
                }
            }
            FatType::FAT16 => {
                let mut data: [u8; 512] = [0; 512];

                let fat_offset = cluster * 2;
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                match self.read(fat_sector as u64, &mut data) {
                    Ok(_) => {}
                    Err(_) => return Err(Error::BlockError),
                };

                let fat: &[u16] =
                    unsafe { core::slice::from_raw_parts(data.as_ptr() as *const u16, 512 / 2) };

                let next_cluster = fat[(offset / 2) as usize];

                if next_cluster >= 0xfff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(next_cluster as u32)
                }
            }
            FatType::FAT32 => {
                let mut data: [u8; 512] = [0; 512];

                let fat_offset = cluster * 4;
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                match self.read(fat_sector as u64, &mut data) {
                    Ok(_) => {}
                    Err(_) => return Err(Error::BlockError),
                };

                let fat: &[u32] =
                    unsafe { core::slice::from_raw_parts(data.as_ptr() as *const u32, 512 / 4) };

                let next_cluster_raw = fat[(offset / 4) as usize];
                let next_cluster = next_cluster_raw & 0x0fffffff;
                if next_cluster >= 0x0ffffff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(next_cluster as u32)
                }
            }

            _ => Err(Error::Unsupported),
        }
    }

    fn directory_find_at_sector(
        &mut self,
        sector: u64,
        name: &str,
    ) -> Result<(FileType, u32, u32), Error> {
        let mut data: [u8; 512] = [0; 512];
        match self.read(sector, &mut data) {
            Ok(_) => {}
            Err(_) => return Err(Error::BlockError),
        };

        let dirs: &[Directory] =
            unsafe { core::slice::from_raw_parts(data.as_ptr() as *const Directory, 512 / 32) };

        for d in dirs {
            // Last entry
            if d.name[0] == 0x0 {
                return Err(Error::EndOfFile);
            }
            // Directory unused
            if d.name[0] == 0xe5 {
                continue;
            }
            // LFN entry
            if d.flags == 0x0f {
                continue;
            }

            let name = name.as_bytes();
            if &d.name[0..name.len()] == name {
                return Ok((
                    if d.flags & 0x10 == 0x10 {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                    (d.cluster_high as u32) << 16 | d.cluster_low as u32,
                    d.size,
                ));
            }
        }

        Err(Error::NotFound)
    }

    pub fn directory_find_at_cluster(
        &mut self,
        cluster: u32,
        name: &str,
    ) -> Result<(FileType, u32, u32), Error> {
        let cluster_start = ((cluster - 2) * self.sectors_per_cluster) + self.first_data_sector;
        for s in 0..self.sectors_per_cluster {
            match self.directory_find_at_sector((s + cluster_start) as u64, name) {
                Ok(r) => return Ok(r),
                Err(Error::NotFound) => {
                    continue;
                }
                Err(Error::EndOfFile) => return Err(Error::NotFound),
                Err(e) => return Err(e),
            }
        }

        match self.next_cluster(cluster) {
            Ok(next_cluster) => self.directory_find_at_cluster(next_cluster, name),
            Err(Error::EndOfFile) => Err(Error::NotFound),
            Err(e) => Err(e),
        }
    }

    pub fn directory_find_at_root(&mut self, name: &str) -> Result<(FileType, u32, u32), Error> {
        match self.fat_type {
            FatType::FAT12 | FatType::FAT16 => {
                let root_directory_start = self.first_data_sector - self.root_dir_sectors;
                for sector in 0..self.root_dir_sectors {
                    let s = (sector + root_directory_start) as u64;
                    match self.directory_find_at_sector(s, name) {
                        Ok(res) => {
                            return Ok(res);
                        }
                        Err(Error::NotFound) => continue,
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                Err(Error::NotFound)
            }
            FatType::FAT32 => self.directory_find_at_cluster(self.root_cluster, name),
            _ => Err(Error::Unsupported),
        }
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

    #[test]
    fn test_fat_directory() {
        let mut d = FakeDisk::new();
        match crate::part::find_efi_partition(&mut d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&mut d, start);
                match f.init() {
                    Ok(()) => {
                        let (ftype, cluster, _) = f.directory_find_at_root("EFI").unwrap();

                        assert_eq!(ftype, super::FileType::Directory);
                        let (ftype, cluster, _) =
                            f.directory_find_at_cluster(cluster, "BOOT").unwrap();
                        assert_eq!(ftype, super::FileType::Directory);
                        let (ftype, cluster, size) =
                            f.directory_find_at_cluster(cluster, "BOOTX64 EFI").unwrap();
                        assert_eq!(ftype, super::FileType::File);
                        assert_eq!(cluster, 4);
                        assert_eq!(size, 133120);
                    }
                    Err(e) => panic!(e),
                }
            }
            Err(e) => panic!(e),
        }
    }
}
