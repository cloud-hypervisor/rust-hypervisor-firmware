// Copyright © 2019 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use r_efi::efi::{AllocateType, MemoryType, PhysicalAddress, Status, VirtualAddress};

const PAGE_SIZE: u64 = 4096;

// Copied from r_efi so we can do Default on it
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MemoryDescriptor {
    pub r#type: u32,
    pub physical_start: PhysicalAddress,
    pub virtual_start: VirtualAddress,
    pub number_of_pages: u64,
    pub attribute: u64,
}

#[derive(Copy, Clone)]
struct Allocation {
    in_use: bool,
    next_allocation: Option<usize>,
    descriptor: MemoryDescriptor,
}

const MAX_ALLOCATIONS: usize = 256;

#[derive(Copy, Clone)]
pub struct Allocator {
    allocations: [Allocation; MAX_ALLOCATIONS],
    key: usize,
    first_allocation: Option<usize>,
}

impl Allocator {
    // Assume called in order with non-overlapping sections.
    pub fn add_initial_allocation(
        &mut self,
        memory_type: MemoryType,
        page_count: u64,
        address: u64,
        attributes: u64,
    ) -> Status {
        self.key += 1;

        if self.first_allocation == None {
            let mut a = &mut self.allocations[0];

            a.in_use = true;
            a.next_allocation = None;
            a.descriptor.r#type = memory_type as u32;
            a.descriptor.number_of_pages = page_count;
            a.descriptor.physical_start = address;
            a.descriptor.attribute = attributes;

            self.first_allocation = Some(0);

            return Status::SUCCESS;
        }

        // Find the last used allocation
        let mut cur = self.first_allocation;
        let mut last = cur.unwrap();
        while cur != None {
            last = cur.unwrap();
            cur = self.allocations[last].next_allocation;
        }

        // Chain the last used allocation to a new allocation
        let next = self.find_free_allocation();

        if next == MAX_ALLOCATIONS {
            return Status::OUT_OF_RESOURCES;
        }

        self.allocations[last].next_allocation = Some(next);

        // And fill in that new allocation
        let next_allocation = &mut self.allocations[next];
        next_allocation.in_use = true;
        next_allocation.next_allocation = None;
        next_allocation.descriptor.r#type = memory_type as u32;
        next_allocation.descriptor.number_of_pages = page_count;
        next_allocation.descriptor.physical_start = address;
        next_allocation.descriptor.attribute = attributes;

        Status::SUCCESS
    }

    fn find_free_allocation(&mut self) -> usize {
        let mut free_allocation = MAX_ALLOCATIONS;
        for i in 0..self.allocations.len() {
            if !self.allocations[i].in_use {
                free_allocation = i;
                break;
            }
        }
        free_allocation
    }

    // Find the descriptor that can contain the desired allocation
    fn find_free_memory(
        &mut self,
        allocation_type: AllocateType,
        page_count: u64,
        address: u64,
    ) -> Option<usize> {
        let mut cur = self.first_allocation;
        while cur != None {
            let a = &mut self.allocations[cur.unwrap()];

            if a.descriptor.r#type != MemoryType::ConventionalMemory as u32 {
                cur = a.next_allocation;
                continue;
            }

            let alloc_bottom = a.descriptor.physical_start;
            let alloc_top = alloc_bottom + PAGE_SIZE * a.descriptor.number_of_pages;

            match allocation_type {
                AllocateType::AllocateAddress => {
                    let req_bottom = address;
                    let req_top = req_bottom + PAGE_SIZE * page_count;

                    if req_bottom >= alloc_bottom && req_top <= alloc_top {
                        return cur;
                    }
                }
                AllocateType::AllocateAnyPages => {
                    // Always allocate generic requests > 1MiB
                    if a.descriptor.number_of_pages >= page_count
                        && a.descriptor.physical_start > 0x10_0000
                    {
                        return cur;
                    }
                }
                AllocateType::AllocateMaxAddress => {
                    let req_bottom = a.descriptor.physical_start;
                    let req_top = req_bottom + PAGE_SIZE * page_count;

                    if a.descriptor.number_of_pages >= page_count && req_top <= address {
                        return cur;
                    }
                }
            }

            cur = a.next_allocation;
        }

        None
    }

    // Splits an allocation preserving all fields with pages in the first half.
    fn split_allocation(&mut self, orig: usize, pages: u64) -> Option<usize> {
        let new = self.find_free_allocation();
        if new == MAX_ALLOCATIONS {
            return None;
        }

        // Copy fields from one being split into new half with exception of pages.
        self.allocations[new].in_use = true;
        self.allocations[new].next_allocation = self.allocations[orig].next_allocation;
        self.allocations[new].descriptor.number_of_pages =
            self.allocations[orig].descriptor.number_of_pages - pages;
        self.allocations[new].descriptor.r#type = self.allocations[orig].descriptor.r#type;
        self.allocations[new].descriptor.attribute = self.allocations[orig].descriptor.attribute;

        // Update details on original
        self.allocations[orig].next_allocation = Some(new);
        self.allocations[orig].descriptor.number_of_pages = pages;

        // Update address on new
        self.allocations[new].descriptor.physical_start =
            self.allocations[orig].descriptor.physical_start + PAGE_SIZE * pages;

        Some(new)
    }

    pub fn allocate_pages(
        &mut self,
        allocate_type: AllocateType,
        memory_type: MemoryType,
        page_count: u64,
        address: u64,
    ) -> (Status, u64) {
        let dest = self.find_free_memory(allocate_type, page_count, address);

        if dest == None {
            return (Status::OUT_OF_RESOURCES, 0);
        }

        self.key += 1;

        let dest = dest.unwrap();

        // Identical special case
        if self.allocations[dest].descriptor.number_of_pages == page_count {
            self.allocations[dest].descriptor.r#type = memory_type as u32;
            return (
                Status::SUCCESS,
                self.allocations[dest].descriptor.physical_start,
            );
        }

        let assigned;
        match allocate_type {
            AllocateType::AllocateAddress => {
                // Most complex: Three cases: at beginning, at end, in the middle.

                // If allocating at the beginning, can just ignore 2nd half as is already marked as free
                if self.allocations[dest].descriptor.physical_start == address {
                    let split = self.split_allocation(dest, page_count);
                    if split == None {
                        return (Status::OUT_OF_RESOURCES, 0);
                    }
                    assigned = dest
                } else {
                    // Work out how pages in the desired address and split at that
                    let pages =
                        (address - self.allocations[dest].descriptor.physical_start) / PAGE_SIZE;
                    let split = self.split_allocation(dest, pages);
                    if split == None {
                        return (Status::OUT_OF_RESOURCES, 0);
                    }
                    let split = split.unwrap();

                    // If second half bigger than we need, split again but ignore that bit
                    if self.allocations[split].descriptor.number_of_pages > page_count {
                        let second_split = self.split_allocation(split, page_count);
                        if second_split == None {
                            return (Status::OUT_OF_RESOURCES, 0);
                        }
                    }

                    assigned = split
                }
            }
            AllocateType::AllocateMaxAddress | AllocateType::AllocateAnyPages => {
                // With the more general allocation we always put at the start of the range
                let split = self.split_allocation(dest, page_count);
                if split == None {
                    return (Status::OUT_OF_RESOURCES, 0);
                }

                assigned = dest;
            }
        }

        self.allocations[assigned].descriptor.r#type = memory_type as u32;
        self.allocations[assigned].descriptor.attribute |= match memory_type {
            MemoryType::RuntimeServicesCode | MemoryType::RuntimeServicesData => {
                r_efi::efi::MEMORY_RUNTIME
            }
            _ => 0,
        };

        (
            Status::SUCCESS,
            self.allocations[assigned].descriptor.physical_start,
        )
    }

    fn merge_free_memory(&mut self) {
        let mut cur = self.first_allocation;

        while cur != None {
            let next_allocation = self.allocations[cur.unwrap()].next_allocation;

            if next_allocation.is_none() {
                return;
            }

            let current = cur.unwrap();
            let next = next_allocation.unwrap();

            // If next allocation has the same type and are contiguous then merge
            if self.allocations[current].descriptor.r#type == MemoryType::ConventionalMemory as u32
                && self.allocations[next].descriptor.r#type == MemoryType::ConventionalMemory as u32
                && self.allocations[next].descriptor.physical_start
                    == self.allocations[current].descriptor.physical_start
                        + self.allocations[current].descriptor.number_of_pages * PAGE_SIZE
            {
                // Add pages into the current one.
                self.allocations[cur.unwrap()].descriptor.number_of_pages +=
                    self.allocations[next].descriptor.number_of_pages;
                // Update next
                self.allocations[current].next_allocation = self.allocations[next].next_allocation;
                // Mark as unused
                self.allocations[next].in_use = false;
            // Keep cur the same so as to handle the case that we've got 3 free blocks in a row
            } else {
                cur = next_allocation;
            }
        }
    }

    pub fn free_pages(&mut self, address: u64) -> Status {
        let mut cur = self.first_allocation;

        while cur != None {
            let a = &mut self.allocations[cur.unwrap()];

            if address == a.descriptor.physical_start {
                a.descriptor.r#type = MemoryType::ConventionalMemory as u32;
                self.merge_free_memory();
                return Status::SUCCESS;
            }
            cur = a.next_allocation;
        }

        Status::NOT_FOUND
    }

    pub fn get_descriptor_count(&self) -> usize {
        let mut count = 0;
        let mut cur = self.first_allocation;

        while cur != None {
            cur = self.allocations[cur.unwrap()].next_allocation;
            count += 1;
        }

        count
    }

    pub fn get_descriptors(&self, out: &mut [MemoryDescriptor]) -> usize {
        assert!(out.len() >= self.get_descriptor_count());

        let mut count = 0;
        let mut cur = self.first_allocation;

        while cur != None {
            out[count] = self.allocations[cur.unwrap()].descriptor;
            cur = self.allocations[cur.unwrap()].next_allocation;
            count += 1;
        }

        count
    }

    pub fn update_virtual_addresses(&mut self, descriptors: &[MemoryDescriptor]) -> Status {
        let mut i = 0;

        'outer: while i < descriptors.len() {
            let mut cur = self.first_allocation;
            while cur != None {
                if self.allocations[cur.unwrap()].descriptor.physical_start
                    == descriptors[i].physical_start
                {
                    self.allocations[cur.unwrap()].descriptor.virtual_start =
                        descriptors[i].virtual_start;
                    i += 1;
                    continue 'outer;
                }

                cur = self.allocations[cur.unwrap()].next_allocation;
            }

            return Status::NOT_FOUND;
        }

        Status::SUCCESS
    }

    pub fn get_map_key(&self) -> usize {
        self.key
    }

    pub const fn new() -> Allocator {
        let allocation = Allocation {
            in_use: false,
            next_allocation: None,
            descriptor: MemoryDescriptor {
                r#type: 0,
                physical_start: 0,
                virtual_start: 0,
                number_of_pages: 0,
                attribute: 0,
            },
        };
        Allocator {
            allocations: [allocation; MAX_ALLOCATIONS],
            key: 0,
            first_allocation: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Allocator;
    use r_efi::efi::{AllocateType, MemoryType, Status};

    fn add_initial_allocations(allocator: &mut Allocator) {
        // Add range 0 - 0x9fc00
        assert_eq!(
            allocator.add_initial_allocation(
                MemoryType::ConventionalMemory,
                0x9fc00 / super::PAGE_SIZE,
                0,
                0
            ),
            Status::SUCCESS
        );

        assert_eq!(allocator.first_allocation, Some(0));
        assert!(allocator.allocations[0].in_use);
        assert_eq!(allocator.allocations[0].next_allocation, None);
        assert_eq!(allocator.allocations[0].descriptor.physical_start, 0);
        assert_eq!(
            allocator.allocations[0].descriptor.number_of_pages,
            0x9fc00 / super::PAGE_SIZE
        );

        // Add range 1 - 128MiB
        assert_eq!(
            allocator.add_initial_allocation(
                MemoryType::ConventionalMemory,
                127 * 1024 * 1024 / super::PAGE_SIZE,
                1024 * 1024,
                0
            ),
            Status::SUCCESS
        );
        assert_eq!(allocator.first_allocation, Some(0));
        assert!(allocator.allocations[0].in_use);
        assert_eq!(allocator.allocations[0].next_allocation, Some(1));
        assert_eq!(allocator.allocations[0].descriptor.physical_start, 0);
        assert_eq!(
            allocator.allocations[0].descriptor.number_of_pages,
            0x9fc00 / super::PAGE_SIZE
        );

        assert!(allocator.allocations[1].in_use);
        assert_eq!(allocator.allocations[1].next_allocation, None);
        assert_eq!(
            allocator.allocations[1].descriptor.physical_start,
            1024 * 1024
        );
        assert_eq!(
            allocator.allocations[1].descriptor.number_of_pages,
            127 * 1024 * 1024 / super::PAGE_SIZE
        );
        assert_eq!(
            allocator.allocations[0].descriptor.r#type,
            MemoryType::ConventionalMemory as u32
        );

        // Add range 3.5GiB to 4.0
        assert_eq!(
            allocator.add_initial_allocation(
                MemoryType::MemoryMappedIO,
                512 * 1024 * 1024 / super::PAGE_SIZE,
                3584 * 1024 * 1024,
                0
            ),
            Status::SUCCESS
        );

        assert!(allocator.allocations[2].in_use);
        assert_eq!(allocator.allocations[2].next_allocation, None);
        assert_eq!(
            allocator.allocations[2].descriptor.physical_start,
            3584 * 1024 * 1024
        );
        assert_eq!(
            allocator.allocations[2].descriptor.number_of_pages,
            512 * 1024 * 1024 / super::PAGE_SIZE
        );
        assert_eq!(
            allocator.allocations[2].descriptor.r#type,
            MemoryType::MemoryMappedIO as u32
        );

        // Add memory from 4GiB to 8GiB of conventional
        assert_eq!(
            allocator.add_initial_allocation(
                MemoryType::ConventionalMemory,
                4096 * 1024 * 1024 / super::PAGE_SIZE,
                4096 * 1024 * 1024,
                0
            ),
            Status::SUCCESS
        );
    }
    #[test]
    fn test_initial_allocations() {
        let mut allocator = Allocator::new();

        assert_eq!(allocator.first_allocation, None);

        add_initial_allocations(&mut allocator);
    }

    #[test]
    fn test_split_allocation() {
        let mut allocator = Allocator::new();

        add_initial_allocations(&mut allocator);

        // Split second range into 1..2MiB, 2..128Mib
        assert_eq!(allocator.split_allocation(1, 1024 * 1024 / 4096), Some(4));

        assert_eq!(allocator.first_allocation, Some(0));
        assert!(allocator.allocations[1].in_use);
        assert_eq!(allocator.allocations[1].next_allocation, Some(4));
        assert_eq!(
            allocator.allocations[1].descriptor.physical_start,
            0x10_0000
        );
        assert_eq!(
            allocator.allocations[1].descriptor.number_of_pages,
            1024 * 1024 / super::PAGE_SIZE
        );
        assert_eq!(
            allocator.allocations[0].descriptor.r#type,
            MemoryType::ConventionalMemory as u32
        );

        assert!(allocator.allocations[4].in_use);
        assert_eq!(allocator.allocations[4].next_allocation, Some(2));
        assert_eq!(
            allocator.allocations[4].descriptor.physical_start,
            2 * 1024 * 1024
        );
        assert_eq!(
            allocator.allocations[4].descriptor.number_of_pages,
            126 * 1024 * 1024 / super::PAGE_SIZE
        );
        assert_eq!(
            allocator.allocations[4].descriptor.r#type,
            MemoryType::ConventionalMemory as u32
        );
    }

    #[test]
    fn test_find_free_allocation() {
        let mut allocator = Allocator::new();

        assert_eq!(allocator.find_free_allocation(), 0);

        add_initial_allocations(&mut allocator);

        assert_eq!(allocator.find_free_allocation(), 4);
    }

    #[test]
    fn test_find_free_memory() {
        let mut allocator = Allocator::new();

        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAddress,
                1024 * 1024 / super::PAGE_SIZE,
                1024 * 1024
            ),
            None
        );

        add_initial_allocations(&mut allocator);

        // 4K at 1MiB
        assert_eq!(
            allocator.find_free_memory(AllocateType::AllocateAddress, 1024, 1024 * 1024),
            Some(1)
        );

        // 1MiB at 1GiB
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAddress,
                1024 * 1024 / super::PAGE_SIZE,
                1024 * 1024 * 1024
            ),
            None
        );

        // 1 GiB at 0
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAddress,
                1024 * 1024 * 1024 / super::PAGE_SIZE,
                0
            ),
            None
        );

        // 2MiB at 127MiB
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAddress,
                2 * 1024 * 1024 / super::PAGE_SIZE,
                127 * 1024 * 1024
            ),
            None
        );

        // Add memory from 4GiB to 8GiB of conventional
        assert_eq!(
            allocator.add_initial_allocation(
                MemoryType::ConventionalMemory,
                4096 * 1024 * 1024 / super::PAGE_SIZE,
                4096 * 1024 * 1024,
                0
            ),
            Status::SUCCESS
        );

        // 64MiB below 4Gib
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateMaxAddress,
                64 * 1024 * 1024 / super::PAGE_SIZE,
                4096 * 1024 * 1024
            ),
            Some(1)
        );

        // 256MiB below 4Gib
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateMaxAddress,
                256 * 1024 * 1024 / super::PAGE_SIZE,
                4096 * 1024 * 1024
            ),
            None,
        );

        // 256MiB below 8Gib
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateMaxAddress,
                256 * 1024 * 1024 / super::PAGE_SIZE,
                8192 * 1024 * 1024
            ),
            Some(3)
        );

        // 128 MiB anywhere
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAnyPages,
                128 * 1024 * 1024 / super::PAGE_SIZE,
                0,
            ),
            Some(3)
        );

        // 256 MiB anywhere
        assert_eq!(
            allocator.find_free_memory(
                AllocateType::AllocateAnyPages,
                256 * 1024 * 1024 / super::PAGE_SIZE,
                0,
            ),
            Some(3)
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_allocate_pages() {
        let mut allocator = Allocator::new();

        add_initial_allocations(&mut allocator);

        let mut descriptors: [super::MemoryDescriptor; super::MAX_ALLOCATIONS] =
            unsafe { std::mem::zeroed() };

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(
                AllocateType::AllocateAddress,
                MemoryType::LoaderData,
                1,
                0x1000
            ),
            (Status::SUCCESS, 0x1000)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, MemoryType::ConventionalMemory as u32);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, MemoryType::LoaderData as u32);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            (0x9fc00 / super::PAGE_SIZE) - 2
        );
        assert_eq!(descriptors[2].r#type, MemoryType::ConventionalMemory as u32);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(
                AllocateType::AllocateAddress,
                MemoryType::LoaderData,
                1,
                0x1000
            ),
            (Status::OUT_OF_RESOURCES, 0)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, MemoryType::ConventionalMemory as u32);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, MemoryType::LoaderData as u32);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            (0x9fc00 / super::PAGE_SIZE) - 2
        );
        assert_eq!(descriptors[2].r#type, MemoryType::ConventionalMemory as u32);

        // 4KiB at 0
        assert_eq!(
            allocator.allocate_pages(AllocateType::AllocateAddress, MemoryType::LoaderData, 1, 0),
            (Status::SUCCESS, 0)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, MemoryType::LoaderData as u32);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, MemoryType::LoaderData as u32);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            (0x9fc00 / super::PAGE_SIZE) - 2
        );
        assert_eq!(descriptors[2].r#type, MemoryType::ConventionalMemory as u32);
    }

    #[test]
    fn test_free_pages() {
        let mut allocator = Allocator::new();

        add_initial_allocations(&mut allocator);

        let mut descriptors: [super::MemoryDescriptor; super::MAX_ALLOCATIONS] =
            unsafe { std::mem::zeroed() };

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(
                AllocateType::AllocateAddress,
                MemoryType::LoaderData,
                1,
                0x1000
            ),
            (Status::SUCCESS, 0x1000)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, MemoryType::ConventionalMemory as u32);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, MemoryType::LoaderData as u32);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            (0x9fc00 / super::PAGE_SIZE) - 2
        );
        assert_eq!(descriptors[2].r#type, MemoryType::ConventionalMemory as u32);

        assert_eq!(allocator.free_pages(0x1000), Status::SUCCESS);

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);
    }
}
