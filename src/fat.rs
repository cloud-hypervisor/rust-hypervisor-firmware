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
struct Header {
    _magic: [u8; 3],
    _identifier: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    root_dir_count: u16,
    legacy_sectors: u16,
    _media_type: u8,
    legacy_sectors_per_fat: u16,
    _sectors_per_track: u16,
    _head_count: u16,
    _hidden_sectors: u32,
    sectors: u32,
}

#[repr(packed)]
struct Fat32Header {
    _header: Header,
    sectors_per_fat: u32,
    _flags: u16,
    _version: u16,
    root_cluster: u32,
    _fsinfo_sector: u16,
    _backup_boot_sector: u16,
    _reserved: [u8; 12],
    _drive_no: u8,
    _nt_flags: u8,
    _signature: u8,
    _serial: u32,
    _volume: [u8; 11],
    _id: [u8; 8],
}

#[repr(packed)]
struct FatDirectory {
    name: [u8; 11],
    flags: u8,
    _unused1: [u8; 8],
    cluster_high: u16,
    _unused2: [u8; 4],
    cluster_low: u16,
    size: u32,
}

#[repr(packed)]
struct FatLongNameEntry {
    seq: u8,
    name: [u16; 5],
    _attr: u8,
    r#_type: u8,
    _checksum: u8,
    name2: [u16; 6],
    _cluster: u16,
    name3: [u16; 2],
}

pub struct DirectoryEntry {
    name: [u8; 11],
    long_name: [u8; 255],
    file_type: FileType,
    size: u32,
    cluster: u32,
}

#[derive(Debug, PartialEq)]
enum FatType {
    Unknown,
    FAT12,
    FAT16,
    FAT32,
}

pub struct Filesystem<'a> {
    device: &'a SectorRead,
    start: u64,
    last: u64,
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

#[derive(Debug, PartialEq)]
pub enum Error {
    BlockError,
    Unsupported,
    NotFound,
    EndOfFile,
    InvalidOffset,
}

#[derive(Debug, PartialEq)]
enum FileType {
    File,
    Directory,
}

pub struct File<'a> {
    filesystem: &'a Filesystem<'a>,
    start_cluster: u32,
    active_cluster: u32,
    sector_offset: u64,
    size: u32,
    position: u32,
}

pub struct Directory<'a> {
    filesystem: &'a Filesystem<'a>,
    cluster: Option<u32>,
    sector: u32,
    offset: usize,
}

fn ucs2_to_ascii(input: &[u16]) -> [u8; 255] {
    let mut output: [u8; 255] = [0; 255];
    let mut i: usize = 0;
    while i < output.len() {
        output[i] = (input[i] & 0xffu16) as u8;
        if output[i] == 0 {
            break;
        }
        i += 1;
    }
    output
}

impl<'a> Directory<'a> {
    // Returns and then increments to point to the next one, may return EndOfFile if this is the last entry
    pub fn next_entry(&mut self) -> Result<DirectoryEntry, Error> {
        let mut long_entry = [0u16; 260];
        loop {
            let mut sector = self.sector;
            if self.cluster.is_some() {
                if self.sector > self.filesystem.sectors_per_cluster {
                    match self.filesystem.next_cluster(self.cluster.unwrap()) {
                        Ok(new_cluster) => {
                            self.cluster = Some(new_cluster);
                            self.sector = 0;
                            self.offset = 0;
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                sector = self.sector
                    + self
                        .filesystem
                        .first_sector_of_cluster(self.cluster.unwrap());
            }

            let mut data: [u8; 512] = [0; 512];
            match self.filesystem.read(sector as u64, &mut data) {
                Ok(_) => {}
                Err(_) => return Err(Error::BlockError),
            };

            let dirs: &[FatDirectory] = unsafe {
                core::slice::from_raw_parts(data.as_ptr() as *const FatDirectory, 512 / 32)
            };

            let lfns: &[FatLongNameEntry] = unsafe {
                core::slice::from_raw_parts(data.as_ptr() as *const FatLongNameEntry, 512 / 32)
            };

            for i in self.offset..dirs.len() {
                let d = &dirs[i];
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
                    // DOS starts sequences as 1. LFN entries come in reverse order before
                    // actual entry so populate the slice using the sequence.
                    let lfn_seq = ((lfns[i].seq & 0x1f) as usize) - 1;
                    let lfn_block = &mut long_entry[lfn_seq * 13..(lfn_seq + 1) * 13];

                    let s = &mut lfn_block[0..5];
                    s.copy_from_slice(unsafe { &lfns[i].name[..] });
                    let s = &mut lfn_block[5..11];
                    s.copy_from_slice(unsafe { &lfns[i].name2[..] });
                    let s = &mut lfn_block[11..13];
                    s.copy_from_slice(unsafe { &lfns[i].name3[..] });

                    continue;
                }

                let entry = DirectoryEntry {
                    name: d.name,
                    file_type: if d.flags & 0x10 == 0x10 {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                    cluster: (d.cluster_high as u32) << 16 | d.cluster_low as u32,
                    size: d.size,
                    long_name: ucs2_to_ascii(&long_entry[..]),
                };

                self.offset = i + 1;
                return Ok(entry);
            }
            self.sector += 1;
            self.offset = 0;
        }
    }
}

pub trait Read {
    fn read(&mut self, data: &mut [u8]) -> Result<u32, Error>;
    fn seek(&mut self, offset: u32) -> Result<(), Error>;
    fn get_size(&self) -> u32;
}

impl<'a> Read for File<'a> {
    fn read(&mut self, data: &mut [u8]) -> Result<u32, Error> {
        assert_eq!(data.len(), 512);

        if self.position >= self.size {
            return Err(Error::EndOfFile);
        }

        if self.sector_offset == self.filesystem.sectors_per_cluster as u64 {
            match self.filesystem.next_cluster(self.active_cluster) {
                Err(e) => {
                    return Err(e);
                }
                Ok(cluster) => {
                    self.active_cluster = cluster;
                    self.sector_offset = 0;
                }
            }
        }

        let cluster_start = self.filesystem.first_sector_of_cluster(self.active_cluster);

        match self
            .filesystem
            .read(cluster_start as u64 + self.sector_offset, data)
        {
            Err(_) => Err(Error::BlockError),
            Ok(()) => {
                self.sector_offset += 1;
                if (self.position + 512) > self.size {
                    let bytes_read = self.size - self.position;
                    self.position = self.size;
                    Ok(bytes_read)
                } else {
                    self.position += 512;
                    Ok(512)
                }
            }
        }
    }

    fn seek(&mut self, position: u32) -> Result<(), Error> {
        if position % 512 != 0 {
            return Err(Error::InvalidOffset);
        }

        if position >= self.size {
            return Err(Error::EndOfFile);
        }

        // Beyond, reset to zero and come back
        if position < self.position {
            self.position = 0;
            self.sector_offset = 0;
            self.active_cluster = self.start_cluster;
        }

        // Like read but without reading, follow cluster chain if we reach end of cluster
        while self.position != position {
            if self.sector_offset == self.filesystem.sectors_per_cluster as u64 {
                match self.filesystem.next_cluster(self.active_cluster) {
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(cluster) => {
                        self.active_cluster = cluster;
                        self.sector_offset = 0;
                    }
                }
            }

            self.sector_offset += 1;
            self.position += 512;
        }

        Ok(())
    }
    fn get_size(&self) -> u32 {
        return self.size;
    }
}

impl<'a> SectorRead for Filesystem<'a> {
    fn read(&self, sector: u64, data: &mut [u8]) -> Result<(), crate::block::Error> {
        if self.start + sector > self.last {
            Err(crate::block::Error::BlockIOError)
        } else {
            self.device.read(self.start + sector, data)
        }
    }
}

impl<'a> Filesystem<'a> {
    pub fn new(device: &'a SectorRead, start: u64, last: u64) -> Filesystem {
        Filesystem {
            device,
            start,
            last,
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

    fn next_cluster(&self, cluster: u32) -> Result<u32, Error> {
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

    fn first_sector_of_cluster(&self, cluster: u32) -> u32 {
        ((cluster - 2) * self.sectors_per_cluster) + self.first_data_sector
    }

    fn root(&self) -> Result<Directory, Error> {
        match self.fat_type {
            FatType::FAT12 | FatType::FAT16 => {
                let root_directory_start = self.first_data_sector - self.root_dir_sectors;
                return Ok(Directory {
                    filesystem: self,
                    cluster: None,
                    sector: root_directory_start,
                    offset: 0,
                });
            }
            FatType::FAT32 => {
                return Ok(Directory {
                    filesystem: self,
                    cluster: Some(self.root_cluster),
                    sector: 0,
                    offset: 0,
                })
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn get_file(&self, cluster: u32, size: u32) -> Result<File, Error> {
        return Ok(File {
            filesystem: self,
            start_cluster: cluster,
            active_cluster: cluster,
            sector_offset: 0,
            size,
            position: 0,
        });
    }

    fn get_directory(&self, cluster: u32) -> Result<Directory, Error> {
        return Ok(Directory {
            filesystem: self,
            cluster: Some(cluster),
            sector: 0,
            offset: 0,
        });
    }

    pub fn open(&self, path: &str) -> Result<File, Error> {
        assert_eq!(path.find('\\'), Some(0));

        let mut residual = path;

        let mut current_dir = self.root().unwrap();
        loop {
            // sub is the directory or file name
            // residual is what is left
            let sub = match &residual[1..].find('\\') {
                None => &residual[1..],
                Some(x) => {
                    // +1 due to above find working on substring
                    let sub = &residual[1..(*x + 1)];
                    residual = &residual[(*x + 1)..];
                    sub
                }
            };

            if sub.len() == 0 {
                return Err(Error::NotFound);
            }

            loop {
                match current_dir.next_entry() {
                    Err(Error::EndOfFile) => return Err(Error::NotFound),
                    Err(e) => return Err(e),
                    Ok(de) => {
                        if (sub.len() <= 11 && &de.name[0..sub.len()] == sub.as_bytes())
                            || &de.long_name[0..sub.len()] == sub.as_bytes()
                        {
                            match de.file_type {
                                FileType::Directory => {
                                    current_dir = self.get_directory(de.cluster).unwrap();
                                    break;
                                }
                                FileType::File => return self.get_file(de.cluster, de.size),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Read;
    use crate::part::tests::FakeDisk;

    #[test]
    fn test_fat_file_reads() {
        let images: [&str; 3] = ["fat12.img", "fat16.img", "fat32.img"];

        for image in &images {
            let mut d = FakeDisk::new(image);

            for n in 9..16 {
                for o in 0..2 {
                    let v = 2u32.pow(n) - o;
                    let len = d.len();
                    let mut fs = crate::fat::Filesystem::new(&mut d, 0, len);
                    fs.init().expect("Error initialising filesystem");
                    let path = format!("\\A\\B\\C\\{}", v);
                    let mut f = fs.open(&path).expect("Error opening file");

                    assert_eq!(f.size, v);

                    let mut bytes_so_far = 0;
                    loop {
                        let mut data: [u8; 512] = [0; 512];
                        match f.read(&mut data) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!(e),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);
                }
            }
        }
    }

    #[test]
    fn test_fat_file_seek() {
        let images: [&str; 3] = ["fat12.img", "fat16.img", "fat32.img"];

        for image in &images {
            let mut d = FakeDisk::new(image);

            for n in 9..16 {
                for o in 0..2 {
                    let v = 2u32.pow(n) - o;
                    let len = d.len();
                    let mut fs = crate::fat::Filesystem::new(&mut d, 0, len);
                    fs.init().expect("Error initialising filesystem");
                    let path = format!("\\A\\B\\C\\{}", v);
                    let mut f = fs.open(&path).expect("Error opening file");

                    assert_eq!(f.size, v);

                    let mut bytes_so_far = 0;
                    loop {
                        let mut data: [u8; 512] = [0; 512];
                        match f.read(&mut data) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!(e),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);

                    f.seek(0).expect("expect seek to work");
                    bytes_so_far = 0;
                    loop {
                        let mut data: [u8; 512] = [0; 512];
                        match f.read(&mut data) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!(e),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);

                    if f.size > 512 && f.size % 2 == 0 {
                        f.seek(f.size / 2).expect("expect seek to work");
                        bytes_so_far = f.size / 2;
                        loop {
                            let mut data: [u8; 512] = [0; 512];
                            match f.read(&mut data) {
                                Ok(bytes) => {
                                    bytes_so_far += bytes;
                                }
                                Err(super::Error::EndOfFile) => {
                                    break;
                                }
                                Err(e) => panic!(e),
                            }
                        }
                        assert_eq!(bytes_so_far, f.size);
                    }
                }
            }
        }
    }

    #[test]
    fn test_fat_init() {
        let mut d = FakeDisk::new("super_grub2_disk_x86_64_efi_2.02s10.iso");
        match crate::part::find_efi_partition(&mut d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&mut d, start, end);
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
    fn test_fat_open() {
        let mut d = FakeDisk::new("super_grub2_disk_x86_64_efi_2.02s10.iso");
        match crate::part::find_efi_partition(&mut d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&mut d, start, end);
                match f.init() {
                    Ok(()) => {
                        let file = f.open("\\EFI\\BOOT\\BOOTX64 EFI").unwrap();
                        assert_eq!(file.active_cluster, 4);
                        assert_eq!(file.size, 133120);
                    }
                    Err(e) => panic!(e),
                }
            }
            Err(e) => panic!(e),
        }
    }

    #[test]
    fn test_fat_list_root() {
        let images: [&str; 3] = ["fat12.img", "fat16.img", "fat32.img"];

        for image in &images {
            let mut disk = FakeDisk::new(image);
            let len = disk.len();
            let mut fs = crate::fat::Filesystem::new(&mut disk, 0, len);
            fs.init().expect("Error initialising filesystem");
            let mut d = fs.root().unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "A          ".as_bytes());
        }
    }
    #[test]
    fn test_fat_list_recurse() {
        let images: [&str; 3] = ["fat12.img", "fat16.img", "fat32.img"];

        for image in &images {
            let mut disk = FakeDisk::new(image);
            let len = disk.len();
            let mut fs = crate::fat::Filesystem::new(&mut disk, 0, len);
            fs.init().expect("Error initialising filesystem");

            let mut d = fs.root().unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "A          ".as_bytes());

            let mut d = fs.get_directory(de.cluster).unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, ".          ".as_bytes());
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "..         ".as_bytes());
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "B          ".as_bytes());
            assert!(d.next_entry().is_err());

            let mut d = fs.get_directory(de.cluster).unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, ".          ".as_bytes());
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "..         ".as_bytes());
            let de = d.next_entry().unwrap();
            assert_eq!(de.name, "C          ".as_bytes());
            assert!(d.next_entry().is_err());
        }
    }

    #[test]
    fn test_fat_long_file_name() {
        let images: [&str; 3] = ["fat12.img", "fat16.img", "fat32.img"];

        for image in &images {
            let mut d = FakeDisk::new(image);
            let len = d.len();
            let mut fs = crate::fat::Filesystem::new(&mut d, 0, len);
            fs.init().expect("Error initialising filesystem");

            assert!(fs.open("\\longfilenametest").is_ok());
        }
    }
}
