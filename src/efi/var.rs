// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2021 Akira Moroo

#[cfg(not(test))]
extern crate alloc;

#[cfg(not(test))]
use alloc::vec::Vec;

use r_efi::efi;

#[derive(Debug)]
struct Descriptor {
    name: Vec<u16>,
    guid: efi::Guid,
    attr: u32,
    data: Vec<u8>,
}

impl Descriptor {
    const fn new() -> Self {
        Self {
            name: Vec::new(),
            guid: efi::Guid::from_fields(0, 0, 0, 0, 0, &[0; 6]),
            attr: 0,
            data: Vec::new(),
        }
    }
}

pub struct VariableAllocator {
    allocations: Vec<Descriptor>,
}

impl VariableAllocator {
    pub const fn new() -> Self {
        Self {
            allocations: Vec::new(),
        }
    }

    fn find(&self, name: *const u16, guid: *const efi::Guid) -> Option<usize> {
        if name.is_null() || guid.is_null() {
            return None;
        }
        let len = crate::common::ucs2_as_ascii_length(name);
        if len == 0 {
            return None;
        }

        let s = unsafe { core::slice::from_raw_parts(name as *const u16, len + 1) };
        let mut name: Vec<u16> = Vec::new();
        name.extend_from_slice(s);
        let guid = unsafe { &*guid };
        for i in 0..self.allocations.len() {
            if name == self.allocations[i].name && guid == &self.allocations[i].guid {
                return Some(i);
            }
        }
        None
    }

    pub fn get(
        &mut self,
        name: *const efi::Char16,
        guid: *const efi::Guid,
        attr: *mut u32,
        size: *mut usize,
        data: *mut core::ffi::c_void,
    ) -> efi::Status {
        if name.is_null() || guid.is_null() || size.is_null() {
            return efi::Status::INVALID_PARAMETER;
        }
        let index = self.find(name, guid);
        if index == None {
            return efi::Status::NOT_FOUND;
        }
        let a = &self.allocations[index.unwrap()];
        unsafe {
            if *size < a.data.len() {
                *size = a.data.len();
                return efi::Status::BUFFER_TOO_SMALL;
            }
        }

        assert!(!a.data.is_empty());
        unsafe {
            if !attr.is_null() {
                *attr = a.attr;
            }
            *size = a.data.len();

            let data = core::slice::from_raw_parts_mut(data as *mut u8, a.data.len());
            data.clone_from_slice(&a.data);
        }

        efi::Status::SUCCESS
    }

    pub fn set(
        &mut self,
        name: *const efi::Char16,
        guid: *const efi::Guid,
        attr: u32,
        size: usize,
        data: *const core::ffi::c_void,
    ) -> efi::Status {
        if name.is_null() || guid.is_null() {
            return efi::Status::INVALID_PARAMETER;
        }
        let len = crate::common::ucs2_as_ascii_length(name);
        if len == 0 {
            return efi::Status::INVALID_PARAMETER;
        }
        let index = self.find(name, guid);
        if index == None {
            // new variable
            if size == 0 {
                return efi::Status::NOT_FOUND;
            }
            if data.is_null() {
                return efi::Status::INVALID_PARAMETER;
            }
            let mut a = Descriptor::new();
            let name = unsafe { core::slice::from_raw_parts(name as *const u16, len + 1) };
            a.name.extend_from_slice(name);
            a.guid = unsafe { *guid };
            a.attr = attr & !efi::VARIABLE_APPEND_WRITE;
            let src = unsafe { core::slice::from_raw_parts(data as *const u8, size) };
            a.data.extend_from_slice(src);

            self.allocations.push(a);

            return efi::Status::SUCCESS;
        }

        if attr & efi::VARIABLE_APPEND_WRITE != 0 {
            // append to existing variable
            if size == 0 {
                return efi::Status::SUCCESS;
            }
            if data.is_null() {
                return efi::Status::INVALID_PARAMETER;
            }
            let a = &mut self.allocations[index.unwrap()];
            let attr = attr & !efi::VARIABLE_APPEND_WRITE;
            if a.attr != attr {
                return efi::Status::INVALID_PARAMETER;
            }
            let src = unsafe { core::slice::from_raw_parts(data as *const u8, size) };
            a.data.extend_from_slice(src);
            return efi::Status::SUCCESS;
        }

        if attr == 0 || size == 0 {
            self.allocations.remove(index.unwrap());
            return efi::Status::SUCCESS;
        }

        let a = &mut self.allocations[index.unwrap()];
        if attr != a.attr {
            return efi::Status::INVALID_PARAMETER;
        }
        a.data.clear();
        let src = unsafe { core::slice::from_raw_parts(data as *const u8, size) };
        a.data.extend_from_slice(src);

        efi::Status::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::VariableAllocator;
    use r_efi::efi;

    const NAME: [efi::Char16; 5] = [116, 101, 115, 116, 0];
    const GUID: efi::Guid = efi::Guid::from_fields(1, 2, 3, 4, 5, &[6; 6]);
    const ATTR: u32 = efi::VARIABLE_BOOTSERVICE_ACCESS | efi::VARIABLE_RUNTIME_ACCESS;

    fn set_initial_variable(allocator: &mut VariableAllocator, data: &[u8]) {
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            ATTR,
            data.len(),
            data.as_ptr() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(allocator.allocations[0].name, NAME);
        assert_eq!(allocator.allocations[0].guid, GUID);
        assert_eq!(allocator.allocations[0].attr, ATTR);
        assert_eq!(allocator.allocations[0].data, data);
    }

    #[test]
    fn test_new() {
        let mut allocator = VariableAllocator::new();
        set_initial_variable(&mut allocator, &[1, 2, 3]);
    }

    #[test]
    fn test_overwrite() {
        let mut allocator = VariableAllocator::new();
        set_initial_variable(&mut allocator, &[1, 2, 3]);

        let data: [u8; 5] = [4, 5, 6, 7, 8];
        let attr = ATTR;
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            attr,
            data.len(),
            data.as_ptr() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(allocator.allocations[0].name, NAME);
        assert_eq!(allocator.allocations[0].guid, GUID);
        assert_eq!(allocator.allocations[0].attr, attr);
        assert_eq!(allocator.allocations[0].data, data);
    }

    #[test]
    fn test_append() {
        let mut allocator = VariableAllocator::new();
        set_initial_variable(&mut allocator, &[1, 2, 3]);

        let size = 0;
        let attr = ATTR | efi::VARIABLE_APPEND_WRITE;
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            attr,
            size,
            core::ptr::null() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(allocator.allocations[0].name, NAME);
        assert_eq!(allocator.allocations[0].guid, GUID);
        assert_eq!(allocator.allocations[0].attr, ATTR);
        assert_eq!(allocator.allocations[0].data, [1, 2, 3]);

        let data: [u8; 5] = [4, 5, 6, 7, 8];
        let attr = ATTR | efi::VARIABLE_APPEND_WRITE;
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            attr,
            data.len(),
            data.as_ptr() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(allocator.allocations[0].name, NAME);
        assert_eq!(allocator.allocations[0].guid, GUID);
        assert_eq!(allocator.allocations[0].attr, ATTR);
        assert_eq!(allocator.allocations[0].data, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_erase() {
        let mut allocator = VariableAllocator::new();
        set_initial_variable(&mut allocator, &[1, 2, 3]);

        let size = 0;
        let attr = ATTR;
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            attr,
            size,
            core::ptr::null() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert!(allocator.allocations.is_empty());

        set_initial_variable(&mut allocator, &[1, 2, 3]);

        let data: [u8; 5] = [4, 5, 6, 7, 8];
        let attr = 0;
        let status = allocator.set(
            NAME.as_ptr(),
            &GUID,
            attr,
            data.len(),
            data.as_ptr() as *const core::ffi::c_void,
        );

        assert_eq!(status, efi::Status::SUCCESS);
        assert!(allocator.allocations.is_empty());
    }

    #[test]
    fn test_get() {
        let mut allocator = VariableAllocator::new();
        const DATA: [u8; 3] = [1, 2, 3];
        set_initial_variable(&mut allocator, &DATA);

        let mut data: [u8; 3] = [0; 3];
        let mut size = data.len();
        let mut attr = 0;
        let status = allocator.get(
            NAME.as_ptr(),
            &GUID,
            &mut attr,
            &mut size,
            data.as_mut_ptr() as *mut core::ffi::c_void,
        );
        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(attr, ATTR);
        assert_eq!(size, DATA.len());
        assert_eq!(data, DATA);

        let mut data: [u8; 3] = [0; 3];
        let mut size = data.len();
        let status = allocator.get(
            NAME.as_ptr(),
            &GUID,
            core::ptr::null_mut() as *mut u32,
            &mut size,
            data.as_mut_ptr() as *mut core::ffi::c_void,
        );
        assert_eq!(status, efi::Status::SUCCESS);
        assert_eq!(size, DATA.len());
        assert_eq!(data, DATA);

        let mut data: [u8; 1] = [0; 1];
        let mut size = data.len();
        let mut attr = 0;
        let status = allocator.get(
            NAME.as_ptr(),
            &GUID,
            &mut attr,
            &mut size,
            data.as_mut_ptr() as *mut core::ffi::c_void,
        );
        assert_eq!(status, efi::Status::BUFFER_TOO_SMALL);
        assert_eq!(attr, 0);
        assert_eq!(size, DATA.len());
        assert_eq!(data, [0; 1]);
    }
}
