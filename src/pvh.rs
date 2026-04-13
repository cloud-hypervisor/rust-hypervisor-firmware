// SPDX-License-Identifier: Apache-2.0
// Copyright 2020 Google LLC

use pvh::start_info::{MemmapTableEntry, StartInfo};

use crate::{
    bootinfo::{EntryType, Info, MemoryEntry},
    common,
    layout::MemoryDescriptor,
};

impl From<MemmapTableEntry> for MemoryEntry {
    fn from(value: MemmapTableEntry) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            entry_type: EntryType::from(value.ty),
        }
    }
}

impl Info for StartInfo {
    fn name(&self) -> &str {
        "PVH Boot Protocol"
    }
    fn rsdp_addr(&self) -> Option<u64> {
        Some(self.rsdp_paddr)
    }
    fn cmdline(&self) -> &[u8] {
        unsafe { common::from_cstring(self.cmdline_paddr) }
    }
    fn num_entries(&self) -> usize {
        // memmap_paddr and memmap_entries only exist in version 1 or later
        if self.version < 1 || self.memmap_paddr == 0 {
            return 0;
        }
        self.memmap_entries as usize
    }
    fn entry(&self, idx: usize) -> MemoryEntry {
        assert!(idx < self.num_entries());
        let ptr = self.memmap_paddr as *const MemmapTableEntry;
        let entry = unsafe { *ptr.add(idx) };
        MemoryEntry::from(entry)
    }
    fn kernel_load_addr(&self) -> u64 {
        crate::arch::x86_64::layout::KERNEL_START
    }
    fn memory_layout(&self) -> &'static [MemoryDescriptor] {
        &crate::arch::x86_64::layout::MEM_LAYOUT[..]
    }
}

// The PVH Boot Protocol starts at the 32-bit entrypoint to our firmware.
extern "C" {
    fn ram32_start() -> !;
}

pvh::xen_elfnote_phys32_entry!(ram32_start);
