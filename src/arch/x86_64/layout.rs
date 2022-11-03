// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use core::{cell::UnsafeCell, ops::Range};

use crate::layout::{MemoryAttribute, MemoryDescriptor, MemoryLayout};

extern "Rust" {
    static ram_min: UnsafeCell<()>;
    static code_start: UnsafeCell<()>;
    static code_end: UnsafeCell<()>;
    static data_start: UnsafeCell<()>;
    static data_end: UnsafeCell<()>;
    static stack_start: UnsafeCell<()>;
    static stack_end: UnsafeCell<()>;
}

pub fn header_range() -> Range<usize> {
    unsafe { (ram_min.get() as _)..(code_start.get() as _) }
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

const NUM_MEM_DESCS: usize = 4;

pub static MEM_LAYOUT: MemoryLayout<NUM_MEM_DESCS> = [
    MemoryDescriptor {
        name: "PVH Header",
        range: header_range,
        attribute: MemoryAttribute::Data,
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
