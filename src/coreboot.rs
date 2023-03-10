// SPDX-License-Identifier: BSD-3-Clause
// Copyright (C) 2020 Akira Moroo
// Copyright (C) 2009 coresystems GmbH
// Copyright (C) 2008 Advanced Micro Devices, Inc.

use core::mem::size_of;

use crate::bootinfo::{EntryType, Info, MemoryEntry};

#[derive(Debug)]
#[repr(C)]
struct Header {
    signature: [u8; 4],
    header_bytes: u32,
    header_checksum: u32,
    table_bytes: u32,
    table_checksum: u32,
    table_entries: u32,
}

#[derive(Debug)]
#[repr(C)]
struct Record {
    tag: u32,
    size: u32,
}

impl Record {
    pub const TAG_FORWARD: u32 = 0x11;
    pub const TAG_MEMORY: u32 = 0x01;
}

#[derive(Debug)]
#[repr(C)]
struct Forward {
    tag: u32,
    size: u32,
    forward: u64,
}

#[derive(Clone, Copy)]
#[repr(packed, C)]
struct MemMapEntry {
    addr: u64,
    size: u64,
    entry_type: u32,
}

impl From<MemMapEntry> for MemoryEntry {
    fn from(value: MemMapEntry) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            entry_type: EntryType::from(value.entry_type),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct StartInfo {
    rsdp_addr: u64,
    memmap_addr: u64,
    memmap_entries: usize,
}

impl Default for StartInfo {
    fn default() -> Self {
        let (memmap_addr, memmap_entries) = match parse_info(0x0, 0x1000) {
            Some((addr, n_entries)) => (addr, n_entries),
            None => match parse_info(0xf0000, 0x1000) {
                Some((addr, n_entries)) => (addr, n_entries),
                None => panic!("coreboot table not found"),
            },
        };
        let ebda_addr = unsafe { *(0x40e as *const u16) };
        let rsdp_addr = match find_rsdp(ebda_addr as u64, 0x400) {
            Some(addr) => addr,
            None => match find_rsdp(0xe0000, 0x20000) {
                Some(addr) => addr,
                None => panic!("RSDP table not found"),
            },
        };
        Self {
            rsdp_addr,
            memmap_addr,
            memmap_entries,
        }
    }
}

impl Info for StartInfo {
    fn name(&self) -> &str {
        "coreboot"
    }
    fn rsdp_addr(&self) -> Option<u64> {
        Some(self.rsdp_addr)
    }
    fn cmdline(&self) -> &[u8] {
        b""
    }
    fn num_entries(&self) -> usize {
        if self.memmap_addr == 0 {
            return 0;
        }
        self.memmap_entries
    }
    fn entry(&self, idx: usize) -> MemoryEntry {
        assert!(idx < self.num_entries());
        let ptr = self.memmap_addr as *const MemMapEntry;
        let entry = unsafe { &*ptr.add(idx) };
        MemoryEntry::from(*entry)
    }
    fn kernel_load_addr(&self) -> u64 {
        crate::arch::x86_64::layout::KERNEL_START
    }
}

fn find_header(start: u64, len: usize) -> Option<u64> {
    const CB_SIGNATURE: u32 = 0x4f49424c;
    for addr in (start..(start + len as u64)).step_by(16) {
        let val = unsafe { *(addr as *const u32) };
        if val == CB_SIGNATURE {
            return Some(addr);
        }
    }
    None
}

fn find_rsdp(start: u64, len: usize) -> Option<u64> {
    const RSDP_SIGNATURE: u64 = 0x2052_5450_2044_5352;
    for addr in (start..(start + len as u64)).step_by(16) {
        let val = unsafe { *(addr as *const u64) };
        if val == RSDP_SIGNATURE {
            return Some(addr);
        }
    }
    None
}

fn parse_info(start: u64, len: usize) -> Option<(u64, usize)> {
    let header_addr = match find_header(start, len) {
        Some(addr) => addr,
        None => {
            return None;
        }
    };
    let header = unsafe { &*(header_addr as *const Header) };
    let ptr = unsafe { (header_addr as *const Header).offset(1) };
    let mut offset = 0;
    for _ in 0..header.table_entries {
        let rec_ptr = unsafe { (ptr as *const u8).offset(offset as isize) };
        let record = unsafe { &(*(rec_ptr as *const Record)) };
        match record.tag {
            Record::TAG_FORWARD => {
                let forward = unsafe { &*(rec_ptr as *const Forward) };
                return parse_info(forward.forward, len);
            }
            Record::TAG_MEMORY => {
                return Some(parse_memmap(record));
            }
            _ => {}
        }
        offset += record.size;
    }
    None
}

fn parse_memmap(record: &Record) -> (u64, usize) {
    assert_eq!(record.tag, Record::TAG_MEMORY);
    let n_entries = record.size as usize / size_of::<MemMapEntry>();
    let rec_size = size_of::<Record>() as isize;
    let rec_ptr = (record as *const Record) as *const u8;
    let mem_ptr = unsafe { rec_ptr.offset(rec_size) as *const MemMapEntry };
    (mem_ptr as u64, n_entries)
}
