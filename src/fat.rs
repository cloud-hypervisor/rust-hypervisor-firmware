// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use crate::{
    block::{Error as BlockError, SectorBuf, SectorRead},
    mem::MemoryRegion,
};

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

impl Default for DirectoryEntry {
    fn default() -> Self {
        DirectoryEntry {
            name: [0; 11],
            long_name: [0; 255],
            file_type: FileType::File,
            size: 0,
            cluster: 0,
        }
    }
}

impl DirectoryEntry {
    pub fn long_name(&self) -> [u8; 255] {
        self.long_name
    }

    pub fn is_file(&self) -> bool {
        matches!(self.file_type, FileType::File)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FatType {
    Unknown,
    FAT12,
    FAT16,
    FAT32,
}

pub struct Filesystem<'a> {
    device: &'a dyn SectorRead,
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
    #[allow(unused)]
    data_cluster_count: u32,
    root_cluster: u32, // FAT32 only
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Block(BlockError),
    Unsupported,
    NotFound,
    EndOfFile,
    InvalidOffset,
    NodeTypeMismatch,
}

#[derive(Debug, PartialEq, Eq)]
enum FileType {
    File,
    Directory,
}

pub enum Node<'a> {
    File(File<'a>),
    Directory(Directory<'a>),
}

impl<'a> From<File<'a>> for Node<'a> {
    fn from(from: File<'a>) -> Node<'a> {
        Node::File(from)
    }
}

impl<'a> From<Directory<'a>> for Node<'a> {
    fn from(from: Directory<'a>) -> Node<'a> {
        Node::Directory(from)
    }
}

impl<'a> TryFrom<Node<'a>> for File<'a> {
    type Error = Error;

    fn try_from(from: Node<'a>) -> Result<Self, Self::Error> {
        match from {
            Node::File(f) => Ok(f),
            _ => Err(Self::Error::NodeTypeMismatch),
        }
    }
}

impl<'a> TryFrom<Node<'a>> for Directory<'a> {
    type Error = Error;

    fn try_from(from: Node<'a>) -> Result<Self, Self::Error> {
        match from {
            Node::Directory(d) => Ok(d),
            _ => Err(Self::Error::NodeTypeMismatch),
        }
    }
}

pub struct File<'a> {
    filesystem: &'a Filesystem<'a>,
    start_cluster: u32,
    active_cluster: u32,
    sector_offset: u64,
    size: u32,
    position: u32,
}

#[derive(Copy, Clone)]
pub struct Directory<'a> {
    filesystem: &'a Filesystem<'a>,
    cluster: Option<u32>,
    first_sector: u32,
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

pub fn is_absolute_path(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with('\\') {
        return true;
    }
    false
}

fn name_to_str(input: &str, output: &mut [u8]) {
    let pat: &[_] = &[' ', '\0'];
    let input = input.trim_matches(pat);
    let len = crate::common::ascii_length(input);
    assert!(len <= output.len());
    if input == "." || input == ".." || len > 12 {
        output[..len].clone_from_slice(input.as_bytes());
        return;
    }

    let mut i = 0;
    for b in output.iter_mut() {
        *b = match input.as_bytes()[i] {
            b'\0' => break,
            b' ' => {
                i = 8;
                b'.'
            }
            c => {
                i += 1;
                c
            }
        };
        if i >= len {
            break;
        }
    }

    // If name is exactly 11 characters long then ensure separator is correctly added
    if (output[10] >= b'a' && output[10] <= b'z' || output[10] >= b'A' && output[10] <= b'Z')
        && output[7] != b' '
        && output[7] != b'.'
    {
        output[11] = output[10];
        output[10] = output[9];
        output[9] = output[8];
        output[8] = b'.';
    }
}

impl<'a> Read for Node<'a> {
    fn read(&mut self, data: &mut [u8]) -> Result<u32, Error> {
        match self {
            Self::File(file) => file.read(data),
            Self::Directory(_) => Err(Error::Unsupported),
        }
    }
    fn seek(&mut self, position: u32) -> Result<(), Error> {
        match self {
            Self::File(file) => file.seek(position),
            Self::Directory(directory) => directory.seek(position),
        }
    }
    fn get_size(&self) -> u32 {
        match self {
            Self::File(file) => file.get_size(),
            Self::Directory(_) => SectorBuf::len() as u32,
        }
    }
}

impl<'a> Directory<'a> {
    fn read_next(&mut self, data: &mut [u8]) -> Result<(), Error> {
        assert_eq!(data.len(), SectorBuf::len());

        let sector = if self.cluster.is_some() {
            if self.sector >= self.filesystem.sectors_per_cluster {
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
            self.sector
                + self
                    .filesystem
                    .first_sector_of_cluster(self.cluster.unwrap())
        } else {
            self.sector
        };

        match self.filesystem.read(u64::from(sector), data) {
            Ok(_) => {}
            Err(e) => return Err(Error::Block(e)),
        };

        Ok(())
    }

    // Checks if there are any other entries.
    pub fn has_next(&mut self) -> Result<bool, Error> {
        let mut data = SectorBuf::new();

        match self.read_next(data.as_mut_bytes()) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        let dirs: &[FatDirectory] = unsafe {
            core::slice::from_raw_parts(
                data.as_bytes().as_ptr() as *const FatDirectory,
                SectorBuf::len() / 32,
            )
        };

        let d = &dirs[self.offset];

        // Last entry
        if d.name[0] == 0x0 {
            return Ok(false);
        }

        Ok(true)
    }

    // Returns and then increments to point to the next one, may return EndOfFile if
    // this is the last entry
    pub fn next_entry(&mut self) -> Result<DirectoryEntry, Error> {
        let mut long_entry = [0u16; 260];
        loop {
            let mut data = SectorBuf::new();

            match self.read_next(data.as_mut_bytes()) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            };

            let num_dirs = SectorBuf::len() / 32;

            let dirs: &[FatDirectory] = unsafe {
                core::slice::from_raw_parts(
                    data.as_bytes().as_ptr() as *const FatDirectory,
                    num_dirs,
                )
            };

            let lfns: &[FatLongNameEntry] = unsafe {
                core::slice::from_raw_parts(
                    data.as_bytes().as_ptr() as *const FatLongNameEntry,
                    num_dirs,
                )
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

                    // Need explicit copy to avoid borrowing packed structure
                    let name = lfns[i].name;
                    let s = &mut lfn_block[0..5];
                    s.copy_from_slice(&name);

                    let name2 = lfns[i].name2;
                    let s = &mut lfn_block[5..11];
                    s.copy_from_slice(&name2);

                    let name3 = lfns[i].name3;
                    let s = &mut lfn_block[11..13];
                    s.copy_from_slice(&name3);

                    continue;
                }

                let entry = DirectoryEntry {
                    name: d.name,
                    file_type: if d.flags & 0x10 == 0x10 {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                    cluster: (u32::from(d.cluster_high)) << 16 | u32::from(d.cluster_low),
                    size: d.size,
                    long_name: ucs2_to_ascii(&long_entry[..]),
                };

                self.offset = i + 1;
                if self.offset >= num_dirs {
                    self.sector += (self.offset / num_dirs) as u32;
                    self.offset %= num_dirs;
                }
                return Ok(entry);
            }
            self.sector += 1;
            self.offset = 0;
        }
    }

    pub fn next_node(&mut self) -> Result<(Node, [u8; 12]), Error> {
        let de = self.next_entry()?;
        let mut name = [0_u8; 12];
        name_to_str(core::str::from_utf8(&de.name).unwrap(), &mut name);

        match de.file_type {
            FileType::Directory => Ok((
                self.filesystem.get_directory(de.cluster).unwrap().into(),
                name,
            )),
            FileType::File => Ok((
                self.filesystem
                    .get_file(de.cluster, de.size)
                    .unwrap()
                    .into(),
                name,
            )),
        }
    }

    pub fn open(&self, path: &str) -> Result<Node, Error> {
        let root = self.filesystem.root().unwrap();
        let dir = if is_absolute_path(path) { &root } else { self };
        self.filesystem.open_from(dir, path)
    }

    pub fn seek(&mut self, offset: u32) -> Result<(), Error> {
        if offset != 0 {
            return Err(Error::Unsupported);
        }
        self.sector = self.first_sector;
        self.offset = 0;
        Ok(())
    }
}

pub trait Read {
    fn read(&mut self, data: &mut [u8]) -> Result<u32, Error>;
    fn seek(&mut self, offset: u32) -> Result<(), Error>;
    fn get_size(&self) -> u32;

    // Loads the remainder of the file into the specified memory region
    fn load_file(&mut self, mem: &mut MemoryRegion) -> Result<(), Error> {
        let mut chunks = mem.as_bytes().chunks_exact_mut(SectorBuf::len());
        for chunk in chunks.by_ref() {
            self.read(chunk)?;
        }
        let last = chunks.into_remainder();
        if last.is_empty() {
            return Ok(());
        }
        // Use tmp buffer for last, partial sector
        let mut dst = SectorBuf::new();
        let bytes = self.read(dst.as_mut_bytes())? as usize;
        assert_eq!(bytes, last.len());
        last.copy_from_slice(&dst.as_bytes()[..bytes]);
        Ok(())
    }
}

impl<'a> Read for File<'a> {
    fn read(&mut self, data: &mut [u8]) -> Result<u32, Error> {
        assert_eq!(data.len(), SectorBuf::len());

        if self.position >= self.size {
            return Err(Error::EndOfFile);
        }

        if self.sector_offset == u64::from(self.filesystem.sectors_per_cluster) {
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
            .read(u64::from(cluster_start) + self.sector_offset, data)
        {
            Err(e) => Err(Error::Block(e)),
            Ok(()) => {
                self.sector_offset += 1;
                let sector_size = SectorBuf::len() as u32;
                if (self.position + sector_size) > self.size {
                    let bytes_read = self.size - self.position;
                    self.position = self.size;
                    Ok(bytes_read)
                } else {
                    self.position += sector_size;
                    Ok(sector_size)
                }
            }
        }
    }

    fn seek(&mut self, position: u32) -> Result<(), Error> {
        let sector_size = SectorBuf::len() as u32;
        if position % sector_size != 0 {
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

        // Like read but without reading, follow cluster chain if we reach end of
        // cluster
        while self.position != position {
            if self.sector_offset == u64::from(self.filesystem.sectors_per_cluster) {
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
            self.position += sector_size;
        }

        Ok(())
    }
    fn get_size(&self) -> u32 {
        self.size
    }
}

impl<'a> SectorRead for Filesystem<'a> {
    fn read(&self, sector: u64, data: &mut [u8]) -> Result<(), crate::block::Error> {
        if self.start + sector > self.last {
            Err(crate::block::Error::BlockIO)
        } else {
            self.device.read(self.start + sector, data)
        }
    }
}

// Do a case-insensitive match on the name with the 8.3 format that you get from
// FAT. In the FAT directory entry the "." isn't stored and any gaps are padded
// with " ".
fn compare_short_name(name: &str, de: &DirectoryEntry) -> bool {
    let name = name.trim_matches(char::from(0));
    // 8.3 (plus 1 for the separator)
    if crate::common::ascii_length(name) > 12 {
        return false;
    }

    let mut i = 0;
    for a in name.as_bytes().iter() {
        // Handle cases which are 11 long but not 8.3 (e.g "loader.conf")
        if i == 11 {
            return false;
        }

        if *a == b'\0' {
            break;
        }

        // Jump to the extension
        if *a == b'.' {
            i = 8;
            continue;
        }

        let b = de.name[i];
        if a.to_ascii_uppercase() != b.to_ascii_uppercase() {
            return false;
        }

        i += 1;
    }
    while i < 11 {
        if de.name[i] != b' ' {
            return false;
        }
        i += 1;
    }
    true
}

fn compare_name(name: &str, de: &DirectoryEntry) -> bool {
    compare_short_name(name, de)
        || (&de.long_name[0..name.len()] == name.as_bytes() && de.long_name[name.len()] == 0)
}

impl<'a> Filesystem<'a> {
    pub fn new(device: &'a dyn SectorRead, start: u64, last: u64) -> Filesystem {
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

        let mut data = SectorBuf::new();
        match self.read(0, data.as_mut_bytes()) {
            Ok(_) => {}
            Err(e) => return Err(Error::Block(e)),
        };

        let h = unsafe { &*(data.as_bytes().as_ptr() as *const Header) };

        self.bytes_per_sector = u32::from(h.bytes_per_sector);
        self.fat_count = u32::from(h.fat_count);
        self.sectors_per_cluster = u32::from(h.sectors_per_cluster);
        self.root_dir_sectors = ((u32::from(h.root_dir_count * 32)) + self.bytes_per_sector - 1)
            / self.bytes_per_sector;

        self.sectors_per_fat = if h.legacy_sectors_per_fat == 0 {
            let h32 = unsafe { &*(data.as_bytes().as_ptr() as *const Fat32Header) };
            h32.sectors_per_fat
        } else {
            u32::from(h.legacy_sectors_per_fat)
        };

        self.sectors = if h.legacy_sectors == 0 {
            h.sectors
        } else {
            u32::from(h.legacy_sectors)
        };

        self.first_fat_sector = u32::from(h.reserved_sectors);
        self.first_data_sector =
            self.first_fat_sector + (self.fat_count * self.sectors_per_fat) + self.root_dir_sectors;
        self.data_sector_count = self.sectors - self.first_data_sector;
        self.data_cluster_count = self.data_sector_count / self.bytes_per_sector;

        self.clusters = self.data_sector_count / u32::from(h.sectors_per_cluster);

        self.fat_type = if self.clusters < FAT12_MAX {
            FatType::FAT12
        } else if self.clusters < FAT16_MAX {
            FatType::FAT16
        } else {
            FatType::FAT32
        };

        if self.fat_type == FatType::FAT32 {
            let h32 = unsafe { &*(data.as_bytes().as_ptr() as *const Fat32Header) };
            self.root_cluster = h32.root_cluster;
        }

        Ok(())
    }

    fn next_cluster(&self, cluster: u32) -> Result<u32, Error> {
        match self.fat_type {
            FatType::FAT12 => {
                let fat_offset = cluster + (cluster / 2); // equivalent of x 1.5
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                let mut data = SectorBuf::new();
                match self.read(u64::from(fat_sector), data.as_mut_bytes()) {
                    Ok(_) => {}
                    Err(e) => return Err(Error::Block(e)),
                };
                let lower_data = data.as_bytes()[offset as usize] as u16;
                let upper_data = if offset < (SectorBuf::len() - 1) as u32 {
                    data.as_bytes()[offset as usize + 1] as u16
                } else {
                    // read next sector to get upper byte if offset is 511
                    match self.read(u64::from(fat_sector) + 1, data.as_mut_bytes()) {
                        Ok(_) => {}
                        Err(e) => return Err(Error::Block(e)),
                    }
                    data.as_bytes()[0] as u16
                };

                let next_cluster_raw = lower_data | (upper_data << 8);

                let next_cluster = if cluster % 2 == 0 {
                    next_cluster_raw & 0xfff
                } else {
                    next_cluster_raw >> 4
                };
                if next_cluster >= 0xff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(u32::from(next_cluster))
                }
            }
            FatType::FAT16 => {
                let mut data = SectorBuf::new();

                let fat_offset = cluster * 2;
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                match self.read(u64::from(fat_sector), data.as_mut_bytes()) {
                    Ok(_) => {}
                    Err(e) => return Err(Error::Block(e)),
                };

                let bytes = data.as_bytes();
                let next_cluster_raw = [bytes[offset as usize], bytes[(offset + 1) as usize]];
                let next_cluster = u16::from_le_bytes(next_cluster_raw);

                if next_cluster >= 0xfff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(u32::from(next_cluster))
                }
            }
            FatType::FAT32 => {
                let mut data = SectorBuf::new();

                let fat_offset = cluster * 4;
                let fat_sector = self.first_fat_sector + (fat_offset / self.bytes_per_sector);
                let offset = fat_offset % self.bytes_per_sector;

                match self.read(u64::from(fat_sector), data.as_mut_bytes()) {
                    Ok(_) => {}
                    Err(e) => return Err(Error::Block(e)),
                };

                let bytes = data.as_bytes();
                let next_cluster_raw = [
                    bytes[offset as usize],
                    bytes[(offset + 1) as usize],
                    bytes[(offset + 2) as usize],
                    bytes[(offset + 3) as usize],
                ];
                let next_cluster_raw = u32::from_le_bytes(next_cluster_raw);
                let next_cluster = next_cluster_raw & 0x0fff_ffff;
                if next_cluster >= 0x0fff_fff8 {
                    Err(Error::EndOfFile)
                } else {
                    Ok(next_cluster)
                }
            }

            _ => Err(Error::Unsupported),
        }
    }

    fn first_sector_of_cluster(&self, cluster: u32) -> u32 {
        ((cluster - 2) * self.sectors_per_cluster) + self.first_data_sector
    }

    pub fn root(&self) -> Result<Directory, Error> {
        match self.fat_type {
            FatType::FAT12 | FatType::FAT16 => {
                let root_directory_start = self.first_data_sector - self.root_dir_sectors;
                Ok(Directory {
                    filesystem: self,
                    cluster: None,
                    first_sector: root_directory_start,
                    sector: root_directory_start,
                    offset: 0,
                })
            }
            FatType::FAT32 => Ok(Directory {
                filesystem: self,
                cluster: Some(self.root_cluster),
                first_sector: 0,
                sector: 0,
                offset: 0,
            }),
            _ => Err(Error::Unsupported),
        }
    }

    fn get_file(&self, cluster: u32, size: u32) -> Result<File, Error> {
        Ok(File {
            filesystem: self,
            start_cluster: cluster,
            active_cluster: cluster,
            sector_offset: 0,
            size,
            position: 0,
        })
    }

    fn get_directory(&self, cluster: u32) -> Result<Directory, Error> {
        Ok(Directory {
            filesystem: self,
            cluster: Some(cluster),
            first_sector: 0,
            sector: 0,
            offset: 0,
        })
    }

    pub fn open(&self, path: &str) -> Result<Node, Error> {
        // path must be absolute path
        assert!(is_absolute_path(path));
        self.open_from(&self.root().unwrap(), path)
    }

    fn open_from(&self, from: &Directory, path: &str) -> Result<Node, Error> {
        let len = crate::common::ascii_length(path);
        assert!(len < 256);
        let mut p = [0_u8; 256];
        let mut residual = if !is_absolute_path(path) {
            p[0] = b'/';
            p[1..1 + len].clone_from_slice(path[..len].as_bytes());
            core::str::from_utf8(&p).unwrap()
        } else {
            path
        };

        let mut current_dir = *from;
        loop {
            current_dir.seek(0)?;

            // sub is the directory or file name
            // residual is what is left
            let sub = match &residual[1..]
                .find('/')
                .or_else(|| (residual[1..]).find('\\'))
            {
                None => {
                    let sub = &residual[1..];
                    residual = "";
                    sub
                }
                Some(x) => {
                    // +1 due to above find working on substring
                    let sub = &residual[1..=*x];
                    residual = &residual[(*x + 1)..];
                    sub
                }
            };

            if sub.is_empty() {
                return Err(Error::NotFound);
            }

            loop {
                match current_dir.next_entry() {
                    Err(Error::EndOfFile) => return Err(Error::NotFound),
                    Err(e) => return Err(e),
                    Ok(de) => {
                        if compare_name(sub, &de) {
                            match de.file_type {
                                FileType::Directory => {
                                    if residual.is_empty() {
                                        return Ok(self.get_directory(de.cluster).unwrap().into());
                                    }
                                    current_dir = self.get_directory(de.cluster).unwrap();
                                    break;
                                }
                                FileType::File => {
                                    return Ok(self.get_file(de.cluster, de.size).unwrap().into())
                                }
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
    use crate::block::SectorBuf;
    use crate::part::tests::*;
    use std::convert::TryInto;
    use std::path::PathBuf;

    fn fat_test_image_paths() -> Vec<PathBuf> {
        let images = ["fat12.img", "fat16.img", "fat32.img"];

        let mut workload_path = dirs::home_dir().unwrap();
        workload_path.push("workloads");

        let mut paths = Vec::<PathBuf>::new();
        for image in &images {
            paths.push(workload_path.join(image))
        }

        paths
    }

    #[test]
    fn test_fat_file_reads() {
        let images = fat_test_image_paths();

        for image in &images {
            let d = FakeDisk::new(&image);

            for n in 9..16 {
                for o in 0..2 {
                    let v = 2u32.pow(n) - o;
                    let len = d.len();
                    let mut fs = crate::fat::Filesystem::new(&d, 0, len);
                    fs.init().expect("Error initialising filesystem");
                    let path = format!("/A/B/C/{v}");
                    let mut f: crate::fat::File = fs
                        .open(&path)
                        .expect("Error opening file")
                        .try_into()
                        .unwrap();

                    assert_eq!(f.size, v);

                    let mut bytes_so_far = 0;
                    loop {
                        let mut data = SectorBuf::new();
                        match f.read(data.as_mut_bytes()) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!("{e:?}"),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);
                }
            }
        }
    }

    #[test]
    fn test_fat_file_seek() {
        let images = fat_test_image_paths();

        for image in &images {
            let d = FakeDisk::new(image);

            for n in 9..16 {
                for o in 0..2 {
                    let v = 2u32.pow(n) - o;
                    let len = d.len();
                    let mut fs = crate::fat::Filesystem::new(&d, 0, len);
                    fs.init().expect("Error initialising filesystem");
                    let path = format!("/A/B/C/{v}");
                    let mut f: crate::fat::File = fs
                        .open(&path)
                        .expect("Error opening file")
                        .try_into()
                        .unwrap();

                    assert_eq!(f.size, v);

                    let mut bytes_so_far = 0;
                    loop {
                        let mut data = SectorBuf::new();
                        match f.read(data.as_mut_bytes()) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!("{e:?}"),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);

                    f.seek(0).expect("expect seek to work");
                    bytes_so_far = 0;
                    loop {
                        let mut data = SectorBuf::new();
                        match f.read(data.as_mut_bytes()) {
                            Ok(bytes) => {
                                bytes_so_far += bytes;
                            }
                            Err(super::Error::EndOfFile) => {
                                break;
                            }
                            Err(e) => panic!("{e:?}"),
                        }
                    }

                    assert_eq!(bytes_so_far, f.size);

                    if f.size > SectorBuf::len() as u32 && f.size % 2 == 0 {
                        f.seek(f.size / 2).expect("expect seek to work");
                        bytes_so_far = f.size / 2;
                        loop {
                            let mut data = SectorBuf::new();
                            match f.read(data.as_mut_bytes()) {
                                Ok(bytes) => {
                                    bytes_so_far += bytes;
                                }
                                Err(super::Error::EndOfFile) => {
                                    break;
                                }
                                Err(e) => panic!("{e:?}"),
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
        let d = FakeDisk::new(&clear_disk_path());
        match crate::part::find_efi_partition(&d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&d, start, end);
                match f.init() {
                    Ok(()) => {
                        assert_eq!(f.sectors, 1_046_528);
                        assert_eq!(f.fat_type, super::FatType::FAT16);
                    }
                    Err(e) => panic!("{e:?}"),
                }
            }
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn test_fat_open() {
        let d = FakeDisk::new(&clear_disk_path());
        match crate::part::find_efi_partition(&d) {
            Ok((start, end)) => {
                let mut f = crate::fat::Filesystem::new(&d, start, end);
                match f.init() {
                    Ok(()) => {
                        let file: crate::fat::File = f
                            .open("\\EFI\\BOOT\\BOOTX64.EFI")
                            .unwrap()
                            .try_into()
                            .unwrap();

                        assert_eq!(file.active_cluster, 166);
                        assert_eq!(file.size, 92789);
                    }
                    Err(e) => panic!("{e:?}"),
                }
            }
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn test_fat_list_root() {
        let images = fat_test_image_paths();

        for image in &images {
            let disk = FakeDisk::new(&image);
            let len = disk.len();
            let mut fs = crate::fat::Filesystem::new(&disk, 0, len);
            fs.init().expect("Error initialising filesystem");
            let mut d = fs.root().unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"A          ");
        }
    }
    #[test]
    fn test_fat_list_recurse() {
        let images = fat_test_image_paths();

        for image in &images {
            let disk = FakeDisk::new(&image);
            let len = disk.len();
            let mut fs = crate::fat::Filesystem::new(&disk, 0, len);
            fs.init().expect("Error initialising filesystem");

            let mut d = fs.root().unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"A          ");

            let mut d = fs.get_directory(de.cluster).unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b".          ");
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"..         ");
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"B          ");
            assert!(d.next_entry().is_err());

            let mut d = fs.get_directory(de.cluster).unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b".          ");
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"..         ");
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"C          ");
            assert!(d.next_entry().is_err());
        }
    }

    #[test]
    fn test_fat_large_directory() {
        let images = fat_test_image_paths();

        for image in &images {
            let disk = FakeDisk::new(&image);
            let len = disk.len();
            let mut fs = crate::fat::Filesystem::new(&disk, 0, len);
            fs.init().expect("Error initialising filesystem");

            let mut d = fs.root().unwrap();
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"A          ");
            let de = d.next_entry().unwrap();
            assert_eq!(&de.name, b"LARGEDIR   ");

            let mut d = fs.get_directory(de.cluster).unwrap();
            while d.has_next().unwrap() {
                d.next_entry().unwrap();
            }
        }
    }

    #[test]
    fn test_fat_long_file_name() {
        let images = fat_test_image_paths();

        for image in &images {
            let d = FakeDisk::new(&image);
            let len = d.len();
            let mut fs = crate::fat::Filesystem::new(&d, 0, len);
            fs.init().expect("Error initialising filesystem");

            assert!(fs.open("/longfilenametest").is_ok());
        }
    }

    #[test]
    fn test_compare_short_name() {
        let mut de = super::DirectoryEntry::default();
        de.name.copy_from_slice(b"X       ABC");
        assert!(super::compare_short_name("X.abc", &de));
        de.name.copy_from_slice(b"ABCDEFGHIJK");
        assert!(super::compare_short_name("abcdefgh.ijk", &de));
        de.name.copy_from_slice(b"EFI-SYSTEM ");
        assert!(!super::compare_short_name("EFI", &de));
    }

    #[test]
    fn test_name_to_str() {
        let mut s = [0_u8; 12];
        super::name_to_str("X       ABC", &mut s);
        assert_eq!(crate::common::ascii_strip(&s), "X.ABC");
        let mut s = [0_u8; 12];
        super::name_to_str(".", &mut s);
        assert_eq!(crate::common::ascii_strip(&s), ".");
        let mut s = [0_u8; 12];
        super::name_to_str("..", &mut s);
        assert_eq!(crate::common::ascii_strip(&s), "..");
        let mut s = [0_u8; 12];
        super::name_to_str("ABCDEFGHIJK", &mut s);
        assert_eq!(crate::common::ascii_strip(&s), "ABCDEFGH.IJK");
    }
}
