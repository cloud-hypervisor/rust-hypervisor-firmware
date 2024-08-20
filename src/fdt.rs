// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use fdt::Fdt;

use crate::{
    bootinfo::{EntryType, Info, MemoryEntry},
    layout::MemoryDescriptor,
};

pub struct StartInfo<'a> {
    acpi_rsdp_addr: Option<u64>,
    fdt_entry: MemoryEntry,
    fdt: Fdt<'a>,
    kernel_load_addr: u64,
    memory_layout: &'static [MemoryDescriptor],
    pci_bar_memory: Option<MemoryEntry>,
}

impl StartInfo<'_> {
    pub fn new(
        ptr: *const u8,
        acpi_rsdp_addr: Option<u64>,
        kernel_load_addr: u64,
        memory_layout: &'static [MemoryDescriptor],
        pci_bar_memory: Option<MemoryEntry>,
    ) -> Self {
        let fdt = unsafe {
            match Fdt::from_ptr(ptr) {
                Ok(fdt) => fdt,
                Err(e) => panic!("Failed to create device tree object: {:?}", e),
            }
        };

        let fdt_entry = MemoryEntry {
            addr: ptr as u64,
            size: fdt.total_size() as u64,
            entry_type: EntryType::Reserved,
        };

        Self {
            fdt_entry,
            fdt,
            acpi_rsdp_addr,
            kernel_load_addr,
            memory_layout,
            pci_bar_memory,
        }
    }

    pub fn find_compatible_region(&self, with: &[&str]) -> Option<(*const u8, usize)> {
        let node = self.fdt.find_compatible(with)?;
        if let Some(region) = node.reg()?.next() {
            return Some((region.starting_address, region.size?));
        }
        None
    }
}

impl Info for StartInfo<'_> {
    fn name(&self) -> &str {
        "FDT"
    }

    fn rsdp_addr(&self) -> Option<u64> {
        self.acpi_rsdp_addr
    }

    fn fdt_reservation(&self) -> Option<MemoryEntry> {
        Some(self.fdt_entry)
    }

    fn cmdline(&self) -> &[u8] {
        match self.fdt.chosen().bootargs() {
            Some(s) => s.as_bytes(),
            None => b"",
        }
    }

    fn num_entries(&self) -> usize {
        let nodes = self.fdt.find_all_nodes("/memory");
        let regions = nodes.flat_map(|n| n.reg().expect("should contain valid memory regions"));
        regions.count()
    }

    fn entry(&self, idx: usize) -> MemoryEntry {
        let nodes = self.fdt.find_all_nodes("/memory");
        let regions = nodes.flat_map(|n| n.reg().expect("should contain valid memory regions"));
        for (i, region) in regions.enumerate() {
            if i == idx {
                return MemoryEntry {
                    addr: region.starting_address as u64,
                    size: region.size.expect("memory size is required") as u64,
                    entry_type: EntryType::Ram,
                };
            }
        }
        panic!("No valid memory entry found");
    }

    fn kernel_load_addr(&self) -> u64 {
        self.kernel_load_addr
    }

    fn memory_layout(&self) -> &'static [MemoryDescriptor] {
        self.memory_layout
    }

    fn pci_bar_memory(&self) -> Option<MemoryEntry> {
        self.pci_bar_memory
    }
}
