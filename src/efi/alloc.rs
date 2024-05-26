// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use r_efi::efi::{self, AllocateType, MemoryDescriptor, MemoryType, Status};

#[derive(Copy, Clone)]
struct Allocation {
    in_use: bool,
    next_allocation: Option<usize>,
    descriptor: MemoryDescriptor,
}

const MAX_ALLOCATIONS: usize = 512;

#[derive(Copy, Clone)]
pub struct Allocator {
    allocations: [Allocation; MAX_ALLOCATIONS],
    key: usize,
    first_allocation: Option<usize>,
    page_size: u64,
}

impl Allocator {
    pub const fn new(page_size: u64) -> Allocator {
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
            page_size,
        }
    }

    pub fn page_count(&self, size: usize) -> u64 {
        ((size + self.page_size as usize - 1) / self.page_size as usize) as u64
    }

    // Assume called in order with non-overlapping sections.
    pub fn add_initial_allocation(
        &mut self,
        memory_type: MemoryType,
        page_count: u64,
        address: u64,
        attributes: u64,
    ) -> Status {
        self.key += 1;

        if self.first_allocation.is_none() {
            let a = &mut self.allocations[0];

            a.in_use = true;
            a.next_allocation = None;
            a.descriptor.r#type = memory_type;
            a.descriptor.number_of_pages = page_count;
            a.descriptor.physical_start = address;
            a.descriptor.attribute = attributes;

            self.first_allocation = Some(0);

            return Status::SUCCESS;
        }

        // Find the last used allocation
        let mut cur = self.first_allocation;
        let mut last = cur.unwrap();
        while cur.is_some() {
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
        next_allocation.descriptor.r#type = memory_type;
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
        while cur.is_some() {
            let a = &mut self.allocations[cur.unwrap()];

            if a.descriptor.r#type != efi::CONVENTIONAL_MEMORY {
                cur = a.next_allocation;
                continue;
            }

            let alloc_bottom = a.descriptor.physical_start;
            let alloc_top = alloc_bottom + self.page_size * a.descriptor.number_of_pages;

            match allocation_type {
                efi::ALLOCATE_ADDRESS => {
                    let req_bottom = address;
                    let req_top = req_bottom + self.page_size * page_count;

                    if req_bottom >= alloc_bottom && req_top <= alloc_top {
                        return cur;
                    }
                }
                efi::ALLOCATE_ANY_PAGES => {
                    // Always allocate generic requests > 1MiB
                    if a.descriptor.number_of_pages >= page_count
                        && a.descriptor.physical_start > 0x10_0000
                    {
                        return cur;
                    }
                }
                efi::ALLOCATE_MAX_ADDRESS => {
                    let req_bottom = a.descriptor.physical_start;
                    let req_top = req_bottom + self.page_size * page_count;

                    if a.descriptor.number_of_pages >= page_count && req_top <= address {
                        return cur;
                    }
                }
                _ => {
                    return None;
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
            self.allocations[orig].descriptor.physical_start + self.page_size * pages;

        Some(new)
    }

    pub fn find_free_pages(
        &mut self,
        allocate_type: AllocateType,
        page_count: u64,
        address: u64,
    ) -> Option<u64> {
        self.find_free_memory(allocate_type, page_count, address)
            .map(|dest| self.allocations[dest].descriptor.physical_start)
    }

    pub fn allocate_pages(
        &mut self,
        allocate_type: AllocateType,
        memory_type: MemoryType,
        page_count: u64,
        address: u64,
    ) -> (Status, u64) {
        let dest = self.find_free_memory(allocate_type, page_count, address);

        if dest.is_none() {
            return (Status::OUT_OF_RESOURCES, 0);
        }

        self.key += 1;

        let dest = dest.unwrap();

        // Identical special case
        if self.allocations[dest].descriptor.number_of_pages == page_count {
            self.allocations[dest].descriptor.r#type = memory_type;
            return (
                Status::SUCCESS,
                self.allocations[dest].descriptor.physical_start,
            );
        }

        let assigned;
        match allocate_type {
            efi::ALLOCATE_ADDRESS => {
                // Most complex: Three cases: at beginning, at end, in the middle.

                // If allocating at the beginning, can just ignore 2nd half as is already marked as free
                if self.allocations[dest].descriptor.physical_start == address {
                    let split = self.split_allocation(dest, page_count);
                    if split.is_none() {
                        return (Status::OUT_OF_RESOURCES, 0);
                    }
                    assigned = dest
                } else {
                    // Work out how pages in the desired address and split at that
                    let pages = (address - self.allocations[dest].descriptor.physical_start)
                        / self.page_size;
                    let split = self.split_allocation(dest, pages);
                    if split.is_none() {
                        return (Status::OUT_OF_RESOURCES, 0);
                    }
                    let split = split.unwrap();

                    // If second half bigger than we need, split again but ignore that bit
                    if self.allocations[split].descriptor.number_of_pages > page_count {
                        let second_split = self.split_allocation(split, page_count);
                        if second_split.is_none() {
                            return (Status::OUT_OF_RESOURCES, 0);
                        }
                    }

                    assigned = split
                }
            }
            efi::ALLOCATE_MAX_ADDRESS | efi::ALLOCATE_ANY_PAGES => {
                // With the more general allocation we always put at the start of the range
                let split = self.split_allocation(dest, page_count);
                if split.is_none() {
                    return (Status::OUT_OF_RESOURCES, 0);
                }

                assigned = dest;
            }
            _ => {
                return (Status::INVALID_PARAMETER, 0);
            }
        }

        self.allocations[assigned].descriptor.r#type = memory_type;
        self.allocations[assigned].descriptor.attribute |= match memory_type {
            efi::RUNTIME_SERVICES_CODE | efi::RUNTIME_SERVICES_DATA => r_efi::efi::MEMORY_RUNTIME,
            _ => 0,
        };

        (
            Status::SUCCESS,
            self.allocations[assigned].descriptor.physical_start,
        )
    }

    fn merge_free_memory(&mut self) {
        let mut cur = self.first_allocation;

        while cur.is_some() {
            let next_allocation = self.allocations[cur.unwrap()].next_allocation;

            if next_allocation.is_none() {
                return;
            }

            let current = cur.unwrap();
            let next = next_allocation.unwrap();

            // If next allocation has the same type and are contiguous then merge
            if self.allocations[current].descriptor.r#type == efi::CONVENTIONAL_MEMORY
                && self.allocations[next].descriptor.r#type == efi::CONVENTIONAL_MEMORY
                && self.allocations[next].descriptor.physical_start
                    == self.allocations[current].descriptor.physical_start
                        + self.allocations[current].descriptor.number_of_pages * self.page_size
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

        while cur.is_some() {
            let a = &mut self.allocations[cur.unwrap()];

            if address == a.descriptor.physical_start {
                a.descriptor.r#type = efi::CONVENTIONAL_MEMORY;
                self.merge_free_memory();
                return Status::SUCCESS;
            }
            cur = a.next_allocation;
        }

        Status::NOT_FOUND
    }

    pub fn allocate_pool(&mut self, memory_type: MemoryType, size: usize) -> (Status, u64) {
        let page_count = (size as u64 + self.page_size - 1) / self.page_size;
        let (status, address) =
            self.allocate_pages(efi::ALLOCATE_ANY_PAGES, memory_type, page_count, 0);

        (status, address)
    }

    pub fn free_pool(&mut self, address: u64) -> Status {
        self.free_pages(address)
    }

    pub fn get_descriptor_count(&self) -> usize {
        let mut count = 0;
        let mut cur = self.first_allocation;

        while cur.is_some() {
            cur = self.allocations[cur.unwrap()].next_allocation;
            count += 1;
        }

        count
    }

    pub fn get_descriptors(&self, out: &mut [MemoryDescriptor]) -> usize {
        assert!(out.len() >= self.get_descriptor_count());

        let mut count = 0;
        let mut cur = self.first_allocation;

        while cur.is_some() {
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
            while cur.is_some() {
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

    pub fn convert_internal_pointer(
        &self,
        descriptors: &[MemoryDescriptor],
        ptr: u64,
    ) -> Option<u64> {
        for descriptor in descriptors.iter() {
            let start = descriptor.physical_start;
            let end = descriptor.physical_start + descriptor.number_of_pages * self.page_size;
            if start <= ptr && ptr < end {
                return Some(ptr - descriptor.physical_start + descriptor.virtual_start);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::Allocator;
    use r_efi::efi::{self, AllocateType, MemoryType, Status};

    const PAGE_SIZE: u64 = crate::layout::MemoryDescriptor::PAGE_SIZE as u64;

    const fn default_descriptor() -> efi::MemoryDescriptor {
        efi::MemoryDescriptor {
            r#type: 0,
            physical_start: 0,
            virtual_start: 0,
            number_of_pages: 0,
            attribute: 0,
        }
    }

    fn add_initial_allocations(allocator: &mut Allocator) {
        // Add range 0 - 0x9fc00
        assert_eq!(
            allocator.add_initial_allocation(
                efi::CONVENTIONAL_MEMORY,
                allocator.page_count(0x9fc00),
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
            allocator.page_count(0x9fc00)
        );

        // Add range 1 - 128MiB
        assert_eq!(
            allocator.add_initial_allocation(
                efi::CONVENTIONAL_MEMORY,
                allocator.page_count(127 * 1024 * 1024),
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
            allocator.page_count(0x9fc00),
        );

        assert!(allocator.allocations[1].in_use);
        assert_eq!(allocator.allocations[1].next_allocation, None);
        assert_eq!(
            allocator.allocations[1].descriptor.physical_start,
            1024 * 1024
        );
        assert_eq!(
            allocator.allocations[1].descriptor.number_of_pages,
            allocator.page_count(127 * 1024 * 1024)
        );
        assert_eq!(
            allocator.allocations[0].descriptor.r#type,
            efi::CONVENTIONAL_MEMORY
        );

        // Add range 3.5GiB to 4.0
        assert_eq!(
            allocator.add_initial_allocation(
                efi::MEMORY_MAPPED_IO,
                allocator.page_count(512 * 1024 * 1024),
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
            allocator.page_count(512 * 1024 * 1024)
        );
        assert_eq!(
            allocator.allocations[2].descriptor.r#type,
            efi::MEMORY_MAPPED_IO
        );

        // Add memory from 4GiB to 8GiB of conventional
        assert_eq!(
            allocator.add_initial_allocation(
                efi::CONVENTIONAL_MEMORY,
                allocator.page_count(4096 * 1024 * 1024),
                4096 * 1024 * 1024,
                0
            ),
            Status::SUCCESS
        );
    }
    #[test]
    fn test_initial_allocations() {
        let mut allocator = Allocator::new(PAGE_SIZE);

        assert_eq!(allocator.first_allocation, None);

        add_initial_allocations(&mut allocator);
    }

    #[test]
    fn test_split_allocation() {
        let mut allocator = Allocator::new(PAGE_SIZE);

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
            allocator.page_count(1024 * 1024)
        );
        assert_eq!(
            allocator.allocations[0].descriptor.r#type,
            efi::CONVENTIONAL_MEMORY
        );

        assert!(allocator.allocations[4].in_use);
        assert_eq!(allocator.allocations[4].next_allocation, Some(2));
        assert_eq!(
            allocator.allocations[4].descriptor.physical_start,
            2 * 1024 * 1024
        );
        assert_eq!(
            allocator.allocations[4].descriptor.number_of_pages,
            allocator.page_count(126 * 1024 * 1024)
        );
        assert_eq!(
            allocator.allocations[4].descriptor.r#type,
            efi::CONVENTIONAL_MEMORY
        );
    }

    #[test]
    fn test_find_free_allocation() {
        let mut allocator = Allocator::new(PAGE_SIZE);

        assert_eq!(allocator.find_free_allocation(), 0);

        add_initial_allocations(&mut allocator);

        assert_eq!(allocator.find_free_allocation(), 4);
    }

    #[test]
    fn test_find_free_memory() {
        let mut allocator = Allocator::new(PAGE_SIZE);

        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ADDRESS,
                allocator.page_count(1024 * 1024),
                1024 * 1024
            ),
            None
        );

        add_initial_allocations(&mut allocator);

        // 4K at 1MiB
        assert_eq!(
            allocator.find_free_memory(efi::ALLOCATE_ADDRESS, 1024, 1024 * 1024),
            Some(1)
        );

        // 1MiB at 1GiB
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ADDRESS,
                allocator.page_count(1024 * 1024),
                1024 * 1024 * 1024
            ),
            None
        );

        // 1 GiB at 0
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ADDRESS,
                allocator.page_count(1024 * 1024 * 1024),
                0
            ),
            None
        );

        // 2MiB at 127MiB
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ADDRESS,
                allocator.page_count(2 * 1024 * 1024),
                127 * 1024 * 1024
            ),
            None
        );

        // Add memory from 4GiB to 8GiB of conventional
        assert_eq!(
            allocator.add_initial_allocation(
                efi::CONVENTIONAL_MEMORY,
                allocator.page_count(4096 * 1024 * 1024),
                4096 * 1024 * 1024,
                0
            ),
            Status::SUCCESS
        );

        // 64MiB below 4Gib
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_MAX_ADDRESS,
                allocator.page_count(64 * 1024 * 1024),
                4096 * 1024 * 1024
            ),
            Some(1)
        );

        // 256MiB below 4Gib
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_MAX_ADDRESS,
                allocator.page_count(256 * 1024 * 1024),
                4096 * 1024 * 1024
            ),
            None,
        );

        // 256MiB below 8Gib
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_MAX_ADDRESS,
                allocator.page_count(256 * 1024 * 1024),
                8192 * 1024 * 1024
            ),
            Some(3)
        );

        // 128 MiB anywhere
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ANY_PAGES,
                allocator.page_count(128 * 1024 * 1024),
                0,
            ),
            Some(3)
        );

        // 256 MiB anywhere
        assert_eq!(
            allocator.find_free_memory(
                efi::ALLOCATE_ANY_PAGES,
                allocator.page_count(256 * 1024 * 1024),
                0,
            ),
            Some(3)
        );
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_allocate_pages() {
        let mut allocator = Allocator::new(PAGE_SIZE);

        add_initial_allocations(&mut allocator);

        let mut descriptors = [default_descriptor(); super::MAX_ALLOCATIONS];

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(efi::ALLOCATE_ADDRESS, efi::LOADER_DATA, 1, 0x1000),
            (Status::SUCCESS, 0x1000)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, efi::CONVENTIONAL_MEMORY);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, efi::LOADER_DATA);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            allocator.page_count(0x9fc00) - 2
        );
        assert_eq!(descriptors[2].r#type, efi::CONVENTIONAL_MEMORY);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(efi::ALLOCATE_ADDRESS, efi::LOADER_DATA, 1, 0x1000),
            (Status::OUT_OF_RESOURCES, 0)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, efi::CONVENTIONAL_MEMORY);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, efi::LOADER_DATA);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            allocator.page_count(0x9fc00) - 2
        );
        assert_eq!(descriptors[2].r#type, efi::CONVENTIONAL_MEMORY);

        // 4KiB at 0
        assert_eq!(
            allocator.allocate_pages(efi::ALLOCATE_ADDRESS, efi::LOADER_DATA, 1, 0),
            (Status::SUCCESS, 0)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, efi::LOADER_DATA);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, efi::LOADER_DATA);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            allocator.page_count(0x9fc00) - 2
        );
        assert_eq!(descriptors[2].r#type, efi::CONVENTIONAL_MEMORY);
    }

    #[test]
    fn test_free_pages() {
        let mut allocator = Allocator::new(PAGE_SIZE);

        add_initial_allocations(&mut allocator);

        let mut descriptors = [default_descriptor(); super::MAX_ALLOCATIONS];

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);

        // 4KiB at 0x1000
        assert_eq!(
            allocator.allocate_pages(efi::ALLOCATE_ADDRESS, efi::LOADER_DATA, 1, 0x1000),
            (Status::SUCCESS, 0x1000)
        );

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 6);

        assert_eq!(descriptors[0].physical_start, 0);
        assert_eq!(descriptors[0].number_of_pages, 1);
        assert_eq!(descriptors[0].r#type, efi::CONVENTIONAL_MEMORY);

        assert_eq!(descriptors[1].physical_start, 0x1000);
        assert_eq!(descriptors[1].number_of_pages, 1);
        assert_eq!(descriptors[1].r#type, efi::LOADER_DATA);

        assert_eq!(descriptors[2].physical_start, 0x2000);
        assert_eq!(
            descriptors[2].number_of_pages,
            allocator.page_count(0x9fc00) - 2
        );
        assert_eq!(descriptors[2].r#type, efi::CONVENTIONAL_MEMORY);

        assert_eq!(allocator.free_pages(0x1000), Status::SUCCESS);

        let count = allocator.get_descriptors(&mut descriptors);

        assert_eq!(count, 4);
    }
}
