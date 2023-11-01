// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use fdt::Fdt;

use crate::{
    bootinfo::{EntryType, Info, MemoryEntry},
    layout::MemoryDescriptor,
};

// Container of kernel image location address and size
#[cfg(target_arch = "aarch64")]
pub struct KernelInfo {
    pub address: u64,
    pub size: u64,
}

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

    // kernel info is a self-defind item that lays inside Chosen node which should be guaranteed by VMM
    #[cfg(target_arch = "aarch64")]
    pub fn find_kernel_info(&self) -> Option<KernelInfo> {
        let chosen = self.fdt.find_node("/chosen").unwrap();
        let address = chosen
            .properties()
            .find(|n| n.name == "linux,kernel-start")
            .map(|n| n.value);

        let addr = match address {
            Some(addr) => {
                let mut a: u64 = 0;
                for p in addr.iter().take(8) {
                    a = (a << 8) + *p as u64;
                }
                a
            }
            None => {
                return None;
            }
        };

        let size = chosen
            .properties()
            .find(|n| n.name == "linux,kernel-size")
            .map(|n| n.value);

        let sz = match size {
            Some(sz) => {
                let mut s: u64 = 0;
                for p in sz.iter().take(8) {
                    s = (s << 8) + *p as u64;
                }
                s
            }
            None => {
                return None;
            }
        };

        Some(KernelInfo {
            address: addr,
            size: sz,
        })
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
        self.fdt.memory().regions().count()
    }

    fn entry(&self, idx: usize) -> MemoryEntry {
        for (i, region) in self.fdt.memory().regions().enumerate() {
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
