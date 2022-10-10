// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo
// Copyright (c) 2021-2022 Andre Richter <andre.o.richter@gmail.com>

use core::ops::RangeInclusive;

use super::paging::*;

pub mod map {
    pub const END: usize = 0x1_0000_0000;

    pub mod fw {
        pub const START: usize = 0x0000_0000;
        pub const END: usize = 0x0040_0000;
    }
    pub mod mmio {
        pub const START: usize = super::fw::END;
        pub const PL011_START: usize = 0x0900_0000;
        pub const PL031_START: usize = 0x0901_0000;
        pub const END: usize = 0x4000_0000;
    }

    pub mod dram {
        const FDT_SIZE: usize = 0x0020_0000;
        const ACPI_SIZE: usize = 0x0020_0000;

        pub const START: usize = super::mmio::END;
        pub const FDT_START: usize = START;
        pub const ACPI_START: usize = FDT_START + FDT_SIZE;
        pub const KERNEL_START: usize = ACPI_START + ACPI_SIZE;
        pub const END: usize = super::END;
    }
}

pub type KernelAddrSpace = AddressSpace<{ map::END }>;

const NUM_MEM_RANGES: usize = 3;

pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    map::END - 1,
    [
        TranslationDescriptor {
            name: "Firmware",
            virtual_range: RangeInclusive::new(map::fw::START, map::fw::END - 1),
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: false,
            },
        },
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
