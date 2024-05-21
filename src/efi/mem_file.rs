// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Rivos Inc.

use core::slice::from_raw_parts;

use crate::{block::SectorBuf, fat};

pub struct MemoryFile {
    address: u64,
    size: u32,
    position: u32,
}

impl MemoryFile {
    pub fn new(address: u64, size: u32) -> Self {
        MemoryFile {
            address,
            size,
            position: 0,
        }
    }
}

impl fat::Read for MemoryFile {
    fn get_size(&self) -> u32 {
        self.size
    }

    fn read(&mut self, data: &mut [u8]) -> Result<u32, fat::Error> {
        let sector_size = SectorBuf::len() as u32;
        assert_eq!(data.len(), SectorBuf::len());

        if (self.position + sector_size) > self.size {
            let bytes_read = self.size - self.position;
            let memory = unsafe {
                from_raw_parts(
                    (self.address + self.position as u64) as *const u8,
                    bytes_read as usize,
                )
            };
            data[0..bytes_read as usize].copy_from_slice(memory);
            self.position = self.size;
            Ok(bytes_read)
        } else {
            let memory = unsafe {
                from_raw_parts(
                    (self.address + self.position as u64) as *const u8,
                    sector_size as usize,
                )
            };
            data[0..sector_size as usize].copy_from_slice(memory);
            self.position += sector_size;
            Ok(sector_size)
        }
    }

    fn seek(&mut self, position: u32) -> Result<(), fat::Error> {
        let sector_size = SectorBuf::len() as u32;
        if position % sector_size != 0 {
            return Err(fat::Error::InvalidOffset);
        }

        if position >= self.size {
            return Err(fat::Error::EndOfFile);
        }

        self.position = position;

        Ok(())
    }
}
