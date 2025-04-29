// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use core::{
    cell::SyncUnsafeCell,
    ffi::c_void,
    mem::{size_of, transmute},
};

use r_efi::{
    efi::{
        self, Boolean, CapsuleHeader, Char16, Guid, MemoryDescriptor, PhysicalAddress, ResetType,
        Status, Time, TimeCapabilities,
    },
    system::{ConfigurationTable, RuntimeServices},
};

use crate::rtc;

use super::{ALLOCATOR, ST, VARIABLES};

pub static mut RS: SyncUnsafeCell<efi::RuntimeServices> =
    SyncUnsafeCell::new(efi::RuntimeServices {
        hdr: efi::TableHeader {
            signature: efi::RUNTIME_SERVICES_SIGNATURE,
            revision: efi::RUNTIME_SERVICES_REVISION,
            header_size: size_of::<efi::RuntimeServices>() as u32,
            crc32: 0, // TODO
            reserved: 0,
        },
        get_time,
        set_time,
        get_wakeup_time,
        set_wakeup_time,
        set_virtual_address_map,
        convert_pointer,
        get_variable,
        get_next_variable_name,
        set_variable,
        get_next_high_mono_count,
        reset_system,
        update_capsule,
        query_capsule_capabilities,
        query_variable_info,
    });

#[allow(clippy::missing_transmute_annotations)]
unsafe fn fixup_at_virtual(descriptors: &[MemoryDescriptor]) {
    #[allow(static_mut_refs)]
    let st = ST.get_mut();
    #[allow(static_mut_refs)]
    let rs = RS.get_mut();

    let ptr = ALLOCATOR
        .borrow()
        .convert_internal_pointer(descriptors, (not_available as *const ()) as u64)
        .unwrap();
    rs.get_time = transmute(ptr);
    rs.set_time = transmute(ptr);
    rs.get_wakeup_time = transmute(ptr);
    rs.set_wakeup_time = transmute(ptr);
    rs.get_variable = transmute(ptr);
    rs.set_variable = transmute(ptr);
    rs.get_next_variable_name = transmute(ptr);
    rs.reset_system = transmute(ptr);
    rs.update_capsule = transmute(ptr);
    rs.query_capsule_capabilities = transmute(ptr);
    rs.query_variable_info = transmute(ptr);

    let ct = st.configuration_table;
    let ptr = ALLOCATOR
        .borrow()
        .convert_internal_pointer(descriptors, (ct as *const _) as u64)
        .unwrap();
    st.configuration_table = ptr as *mut ConfigurationTable;

    let rs = st.runtime_services;
    let ptr = ALLOCATOR
        .borrow()
        .convert_internal_pointer(descriptors, (rs as *const _) as u64)
        .unwrap();
    st.runtime_services = ptr as *mut RuntimeServices;
}

pub extern "efiapi" fn not_available() -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn get_time(time: *mut Time, _: *mut TimeCapabilities) -> Status {
    if time.is_null() {
        return Status::INVALID_PARAMETER;
    }

    let (year, month, day) = match rtc::read_date() {
        Ok((y, m, d)) => (y, m, d),
        Err(()) => return Status::DEVICE_ERROR,
    };
    let (hour, minute, second) = match rtc::read_time() {
        Ok((h, m, s)) => (h, m, s),
        Err(()) => return Status::DEVICE_ERROR,
    };

    unsafe {
        (*time).year = 2000 + year as u16;
        (*time).month = month;
        (*time).day = day;
        (*time).hour = hour;
        (*time).minute = minute;
        (*time).second = second;
        (*time).nanosecond = 0;
        (*time).timezone = 0;
        (*time).daylight = 0;
    }

    Status::SUCCESS
}

pub extern "efiapi" fn set_time(_: *mut Time) -> Status {
    Status::DEVICE_ERROR
}

pub extern "efiapi" fn get_wakeup_time(_: *mut Boolean, _: *mut Boolean, _: *mut Time) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn set_wakeup_time(_: Boolean, _: *mut Time) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn set_virtual_address_map(
    map_size: usize,
    descriptor_size: usize,
    version: u32,
    descriptors: *mut MemoryDescriptor,
) -> Status {
    let count = map_size / descriptor_size;

    if version != efi::MEMORY_DESCRIPTOR_VERSION {
        return Status::INVALID_PARAMETER;
    }

    let descriptors = unsafe { core::slice::from_raw_parts_mut(descriptors, count) };

    unsafe {
        fixup_at_virtual(descriptors);
    }

    ALLOCATOR.borrow_mut().update_virtual_addresses(descriptors)
}

pub extern "efiapi" fn convert_pointer(_: usize, _: *mut *mut c_void) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn get_variable(
    variable_name: *mut Char16,
    vendor_guid: *mut Guid,
    attributes: *mut u32,
    data_size: *mut usize,
    data: *mut c_void,
) -> Status {
    if cfg!(feature = "efi-var") {
        VARIABLES
            .borrow_mut()
            .get(variable_name, vendor_guid, attributes, data_size, data)
    } else {
        Status::NOT_FOUND
    }
}

pub extern "efiapi" fn get_next_variable_name(
    _: *mut usize,
    _: *mut Char16,
    _: *mut Guid,
) -> Status {
    Status::NOT_FOUND
}

pub extern "efiapi" fn set_variable(
    variable_name: *mut Char16,
    vendor_guid: *mut Guid,
    attributes: u32,
    data_size: usize,
    data: *mut c_void,
) -> Status {
    if cfg!(feature = "efi-var") {
        VARIABLES
            .borrow_mut()
            .set(variable_name, vendor_guid, attributes, data_size, data)
    } else {
        Status::UNSUPPORTED
    }
}

pub extern "efiapi" fn get_next_high_mono_count(_: *mut u32) -> Status {
    Status::DEVICE_ERROR
}

pub extern "efiapi" fn reset_system(_: ResetType, _: Status, _: usize, _: *mut c_void) {
    // Don't do anything to force the kernel to use ACPI for shutdown and triple-fault for reset
}

pub extern "efiapi" fn update_capsule(
    _: *mut *mut CapsuleHeader,
    _: usize,
    _: PhysicalAddress,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn query_capsule_capabilities(
    _: *mut *mut CapsuleHeader,
    _: usize,
    _: *mut u64,
    _: *mut ResetType,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn query_variable_info(
    _: u32,
    max_storage: *mut u64,
    remaining_storage: *mut u64,
    max_size: *mut u64,
) -> Status {
    unsafe {
        *max_storage = 0;
        *remaining_storage = 0;
        *max_size = 0;
    }
    Status::SUCCESS
}
