// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use crate::layout::MemoryDescriptor;

// Common data needed for all boot paths
pub trait Info {
    // Name of for this boot protocol
    fn name(&self) -> &str;
    // Starting address of the Root System Descriptor Pointer
    fn rsdp_addr(&self) -> Option<u64> {
        None
    }
    // Address of FDT to use for booting if present
    fn fdt_addr(&self) -> Option<u64> {
        None
    }
    // The kernel command line (not including null terminator)
    fn cmdline(&self) -> &[u8];
    // Methods to access the Memory map
    fn num_entries(&self) -> usize;
    fn entry(&self, idx: usize) -> MemoryEntry;
    // Where to load kernel
    fn kernel_load_addr(&self) -> u64;
    // Reference to memory layout
    fn memory_layout(&self) -> &'static [MemoryDescriptor];
    // MMIO address space that can be used for PCI BARs if needed
    fn pci_bar_memory(&self) -> Option<MemoryEntry> {
        None
    }
}

#[derive(Clone, Copy)]
pub struct MemoryEntry {
    pub addr: u64,
    pub size: u64,
    pub entry_type: EntryType,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EntryType {
    Ram,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    Bad,
    VendorReserved,
    CorebootTable,
}
