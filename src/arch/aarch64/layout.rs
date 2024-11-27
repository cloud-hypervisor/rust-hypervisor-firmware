// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo
// Copyright (c) 2021-2022 Andre Richter <andre.o.richter@gmail.com>

use core::{
    cell::UnsafeCell,
    ops::{Range, RangeInclusive},
};

use crate::layout::{MemoryAttribute, MemoryDescriptor, MemoryLayout};

use super::paging::*;

extern "Rust" {
    static code_start: UnsafeCell<()>;
    static code_end: UnsafeCell<()>;
    static data_start: UnsafeCell<()>;
    static data_end: UnsafeCell<()>;
    static stack_start: UnsafeCell<()>;
    static stack_end: UnsafeCell<()>;
}

pub mod map {
    // Create page table for 2T
    pub const END: usize = 0x20_000_000_000;

    // Firmware region won't be used by this firmware, so merge it into mmio region
    // is harmless and better for management.
    pub mod mmio {
        pub const START: usize = 0x0000_0000;
        pub const PL011_START: usize = 0x0900_0000;
        pub const PL031_START: usize = 0x0901_0000;
        pub const END: usize = 0x4000_0000;
    }

    pub mod dram {
        pub const FDT_SIZE: usize = 0x0020_0000;
        pub const ACPI_SIZE: usize = 0x0020_0000;

        pub const START: usize = super::mmio::END;
        pub const FDT_START: usize = START;
        pub const ACPI_START: usize = FDT_START + FDT_SIZE;
        pub const KERNEL_START: usize = ACPI_START + ACPI_SIZE;
        pub const END: usize = super::END;
    }
}

pub type KernelAddrSpace = AddressSpace<{ map::END }>;

const NUM_MEM_RANGES: usize = 2;

pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    map::END - 1,
    [
        TranslationDescriptor {
            name: "Device MMIO",
            virtual_range: RangeInclusive::new(map::mmio::START, map::mmio::END - 1),
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
        TranslationDescriptor {
            name: "System Memory",
            virtual_range: RangeInclusive::new(map::dram::START, map::dram::END - 1),
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadWrite, // FIXME
                execute_never: false,
            },
        },
    ],
);

pub fn virt_mem_layout() -> &'static KernelVirtualLayout<NUM_MEM_RANGES> {
    &LAYOUT
}

pub fn mmio_range() -> Range<usize> {
    map::mmio::START..map::mmio::END
}

pub fn reserved_range() -> Range<usize> {
    map::dram::START..map::dram::KERNEL_START
}

pub fn code_range() -> Range<usize> {
    unsafe { (code_start.get() as _)..(code_end.get() as _) }
}

pub fn data_range() -> Range<usize> {
    unsafe { (data_start.get() as _)..(data_end.get() as _) }
}

pub fn stack_range() -> Range<usize> {
    unsafe { (stack_start.get() as _)..(stack_end.get() as _) }
}

const NUM_MEM_DESCS: usize = 5;

pub static MEM_LAYOUT: MemoryLayout<NUM_MEM_DESCS> = [
    MemoryDescriptor {
        name: "MMIO",
        range: mmio_range,
        attribute: MemoryAttribute::Mmio,
    },
    MemoryDescriptor {
        name: "Reserved",
        range: reserved_range,
        attribute: MemoryAttribute::Unusable,
    },
    MemoryDescriptor {
        name: "Code",
        range: code_range,
        attribute: MemoryAttribute::Code,
    },
    MemoryDescriptor {
        name: "Data",
        range: data_range,
        attribute: MemoryAttribute::Data,
    },
    MemoryDescriptor {
        name: "Stack",
        range: stack_range,
        attribute: MemoryAttribute::Data,
    },
];
