// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo
// Copyright (c) 2021-2022 Andre Richter <andre.o.richter@gmail.com>
// Copyright (C) 2023 Rivos Inc.

use core::{cell::UnsafeCell, ops::Range};

use crate::layout::{MemoryAttribute, MemoryDescriptor, MemoryLayout};

unsafe extern "Rust" {
    unsafe static code_start: UnsafeCell<()>;
    unsafe static code_end: UnsafeCell<()>;
    unsafe static data_start: UnsafeCell<()>;
    unsafe static data_end: UnsafeCell<()>;
    unsafe static stack_start: UnsafeCell<()>;
    unsafe static stack_end: UnsafeCell<()>;
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

pub fn reserved_range() -> Range<usize> {
    0x8000_0000..0x8020_0000
}

const NUM_MEM_DESCS: usize = 4;

pub static MEM_LAYOUT: MemoryLayout<NUM_MEM_DESCS> = [
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
    MemoryDescriptor {
        name: "SBI",
        range: reserved_range,
        attribute: MemoryAttribute::Unusable,
    },
];
