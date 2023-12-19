// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

#[cfg(all(not(test), not(feature = "integration_tests")))]
use core::alloc as heap_alloc;

use core::{
    ffi::c_void,
    mem::{size_of, transmute},
    ptr::null_mut,
};

use atomic_refcell::AtomicRefCell;
use linked_list_allocator::LockedHeap;
use r_efi::{
    efi::{
        self, AllocateType, Boolean, CapsuleHeader, Char16, Event, EventNotify, Guid, Handle,
        InterfaceType, LocateSearchType, MemoryDescriptor, MemoryType,
        OpenProtocolInformationEntry, PhysicalAddress, ResetType, Status, Time, TimeCapabilities,
        TimerDelay, Tpl,
    },
    protocols::{
        device_path::Protocol as DevicePathProtocol, loaded_image::Protocol as LoadedImageProtocol,
    },
    system::{ConfigurationTable, RuntimeServices},
};

#[cfg(target_arch = "riscv64")]
use r_efi::{eficall, eficall_abi};

use crate::bootinfo;
use crate::layout;
use crate::rtc;

mod alloc;
mod block;
mod console;
mod file;
mod var;

use alloc::Allocator;
use var::VariableAllocator;
#[cfg(target_arch = "aarch64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTAA64.EFI";
#[cfg(target_arch = "x86_64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTX64.EFI";
#[cfg(target_arch = "riscv64")]
pub const EFI_BOOT_PATH: &str = "\\EFI\\BOOT\\BOOTRISCV64.EFI";

#[derive(Copy, Clone, PartialEq, Eq)]
enum HandleType {
    None,
    Block,
    FileSystem,
    LoadedImage,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct HandleWrapper {
    handle_type: HandleType,
}

pub static ALLOCATOR: AtomicRefCell<Allocator> = AtomicRefCell::new(Allocator::new());

#[cfg(not(test))]
#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(all(not(test), not(feature = "integration_tests")))]
#[alloc_error_handler]
fn heap_alloc_error_handler(layout: heap_alloc::Layout) -> ! {
    panic!("heap allocation error: {:?}", layout);
}

pub static VARIABLES: AtomicRefCell<VariableAllocator> =
    AtomicRefCell::new(VariableAllocator::new());

static mut RS: efi::RuntimeServices = efi::RuntimeServices {
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
};

static mut BS: efi::BootServices = efi::BootServices {
    hdr: efi::TableHeader {
        signature: efi::BOOT_SERVICES_SIGNATURE,
        revision: efi::BOOT_SERVICES_REVISION,
        header_size: size_of::<efi::BootServices>() as u32,
        crc32: 0, // TODO
        reserved: 0,
    },
    raise_tpl,
    restore_tpl,
    allocate_pages,
    free_pages,
    get_memory_map,
    allocate_pool,
    free_pool,
    create_event,
    set_timer,
    wait_for_event,
    signal_event,
    close_event,
    check_event,
    install_protocol_interface,
    reinstall_protocol_interface,
    uninstall_protocol_interface,
    handle_protocol,
    register_protocol_notify,
    locate_handle,
    locate_device_path,
    install_configuration_table,
    load_image,
    start_image,
    exit,
    unload_image,
    exit_boot_services,
    get_next_monotonic_count,
    stall,
    set_watchdog_timer,
    connect_controller,
    disconnect_controller,
    open_protocol,
    close_protocol,
    open_protocol_information,
    protocols_per_handle,
    locate_handle_buffer,
    locate_protocol,
    install_multiple_protocol_interfaces,
    uninstall_multiple_protocol_interfaces,
    calculate_crc32,
    copy_mem,
    set_mem,
    create_event_ex,
    reserved: null_mut(),
};

const INVALID_GUID: Guid = Guid::from_fields(0, 0, 0, 0, 0, &[0_u8; 6]);
const MAX_CT_ENTRIES: usize = 8;
static mut CT: [efi::ConfigurationTable; MAX_CT_ENTRIES] = [efi::ConfigurationTable {
    vendor_guid: INVALID_GUID,
    vendor_table: null_mut(),
}; MAX_CT_ENTRIES];

// RHF string in UCS-2
const FIRMWARE_STRING: [u16; 4] = [0x0052, 0x0048, 0x0046, 0x0000];

static mut ST: efi::SystemTable = efi::SystemTable {
    hdr: efi::TableHeader {
        signature: efi::SYSTEM_TABLE_SIGNATURE,
        revision: (2 << 16) | (80),
        header_size: size_of::<efi::SystemTable>() as u32,
        crc32: 0, // TODO
        reserved: 0,
    },
    firmware_vendor: FIRMWARE_STRING.as_ptr() as *mut u16,
    firmware_revision: 0,
    console_in_handle: console::STDIN_HANDLE,
    con_in: null_mut(),
    console_out_handle: console::STDOUT_HANDLE,
    con_out: null_mut(),
    standard_error_handle: console::STDERR_HANDLE,
    std_err: null_mut(),
    runtime_services: null_mut(),
    boot_services: null_mut(),
    number_of_table_entries: 0,
    configuration_table: null_mut(),
};

static mut BLOCK_WRAPPERS: block::BlockWrappers = block::BlockWrappers {
    wrappers: [null_mut(); 16],
    count: 0,
};

fn convert_internal_pointer(descriptors: &[alloc::MemoryDescriptor], ptr: u64) -> Option<u64> {
    for descriptor in descriptors.iter() {
        let start = descriptor.physical_start;
        let end = descriptor.physical_start + descriptor.number_of_pages * PAGE_SIZE;
        if start <= ptr && ptr < end {
            return Some(ptr - descriptor.physical_start + descriptor.virtual_start);
        }
    }
    None
}

unsafe fn fixup_at_virtual(descriptors: &[alloc::MemoryDescriptor]) {
    let st = &mut ST;
    let rs = &mut RS;

    let ptr = convert_internal_pointer(descriptors, (not_available as *const ()) as u64).unwrap();
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
    let ptr = convert_internal_pointer(descriptors, (ct as *const _) as u64).unwrap();
    st.configuration_table = ptr as *mut ConfigurationTable;

    let rs = st.runtime_services;
    let ptr = convert_internal_pointer(descriptors, (rs as *const _) as u64).unwrap();
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

    let descriptors = unsafe {
        core::slice::from_raw_parts_mut(descriptors as *mut alloc::MemoryDescriptor, count)
    };

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

pub extern "efiapi" fn raise_tpl(_: Tpl) -> Tpl {
    0
}

pub extern "efiapi" fn restore_tpl(_: Tpl) {}

pub extern "efiapi" fn allocate_pages(
    allocate_type: AllocateType,
    memory_type: MemoryType,
    pages: usize,
    address: *mut PhysicalAddress,
) -> Status {
    let (status, new_address) =
        ALLOCATOR
            .borrow_mut()
            .allocate_pages(allocate_type, memory_type, pages as u64, unsafe {
                *address
            });
    if status == Status::SUCCESS {
        unsafe {
            *address = new_address;
        }
    }
    status
}

pub extern "efiapi" fn free_pages(address: PhysicalAddress, _: usize) -> Status {
    ALLOCATOR.borrow_mut().free_pages(address)
}

pub extern "efiapi" fn get_memory_map(
    memory_map_size: *mut usize,
    out: *mut MemoryDescriptor,
    key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> Status {
    if memory_map_size.is_null() {
        return Status::INVALID_PARAMETER;
    }

    if !descriptor_size.is_null() {
        unsafe {
            *descriptor_size = size_of::<MemoryDescriptor>();
        }
    }

    if !descriptor_version.is_null() {
        unsafe {
            *descriptor_version = efi::MEMORY_DESCRIPTOR_VERSION;
        }
    }

    let count = ALLOCATOR.borrow().get_descriptor_count();
    let map_size = size_of::<MemoryDescriptor>() * count;
    if unsafe { *memory_map_size } < map_size {
        unsafe {
            *memory_map_size = map_size;
        }
        return Status::BUFFER_TOO_SMALL;
    }

    if key.is_null() {
        return Status::INVALID_PARAMETER;
    }

    let out =
        unsafe { core::slice::from_raw_parts_mut(out as *mut alloc::MemoryDescriptor, count) };
    let count = ALLOCATOR.borrow().get_descriptors(out);
    let map_size = size_of::<MemoryDescriptor>() * count;
    unsafe {
        *memory_map_size = map_size;
        *key = ALLOCATOR.borrow().get_map_key();
    }

    Status::SUCCESS
}

pub extern "efiapi" fn allocate_pool(
    memory_type: MemoryType,
    size: usize,
    address: *mut *mut c_void,
) -> Status {
    let (status, new_address) = ALLOCATOR.borrow_mut().allocate_pages(
        efi::ALLOCATE_ANY_PAGES,
        memory_type,
        ((size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize) as u64,
        address as u64,
    );

    if status == Status::SUCCESS {
        unsafe {
            *address = new_address as *mut c_void;
        }
    }

    status
}

pub extern "efiapi" fn free_pool(ptr: *mut c_void) -> Status {
    ALLOCATOR.borrow_mut().free_pages(ptr as u64)
}

pub extern "efiapi" fn create_event(
    _: u32,
    _: Tpl,
    _: Option<EventNotify>,
    _: *mut c_void,
    _: *mut Event,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn set_timer(_: Event, _: TimerDelay, _: u64) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn wait_for_event(_: usize, _: *mut Event, _: *mut usize) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn signal_event(_: Event) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn close_event(_: Event) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn check_event(_: Event) -> Status {
    Status::UNSUPPORTED
}

const SHIM_LOCK_PROTOCOL_GUID: Guid = Guid::from_fields(
    0x605d_ab50,
    0xe046,
    0x4300,
    0xab,
    0xb6,
    &[0x3d, 0xd8, 0x10, 0xdd, 0x8b, 0x23],
);

pub extern "efiapi" fn install_protocol_interface(
    _: *mut Handle,
    guid: *mut Guid,
    _: InterfaceType,
    _: *mut c_void,
) -> Status {
    if unsafe { *guid } == SHIM_LOCK_PROTOCOL_GUID {
        Status::SUCCESS
    } else {
        Status::UNSUPPORTED
    }
}

pub extern "efiapi" fn reinstall_protocol_interface(
    _: Handle,
    _: *mut Guid,
    _: *mut c_void,
    _: *mut c_void,
) -> Status {
    Status::NOT_FOUND
}

pub extern "efiapi" fn uninstall_protocol_interface(
    _: Handle,
    _: *mut Guid,
    _: *mut c_void,
) -> Status {
    Status::NOT_FOUND
}

pub extern "efiapi" fn handle_protocol(
    handle: Handle,
    guid: *mut Guid,
    out: *mut *mut c_void,
) -> Status {
    open_protocol(handle, guid, out, null_mut(), null_mut(), 0)
}

pub extern "efiapi" fn register_protocol_notify(
    _: *mut Guid,
    _: Event,
    _: *mut *mut c_void,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn locate_handle(
    _: LocateSearchType,
    guid: *mut Guid,
    _: *mut c_void,
    size: *mut usize,
    handles: *mut Handle,
) -> Status {
    if unsafe { *guid } == block::PROTOCOL_GUID {
        let count = unsafe { BLOCK_WRAPPERS.count };
        if unsafe { *size } < size_of::<Handle>() * count {
            unsafe { *size = size_of::<Handle>() * count };
            return Status::BUFFER_TOO_SMALL;
        }

        let handles =
            unsafe { core::slice::from_raw_parts_mut(handles, *size / size_of::<Handle>()) };

        let wrappers_as_handles: &[Handle] = unsafe {
            core::slice::from_raw_parts_mut(
                BLOCK_WRAPPERS.wrappers.as_mut_ptr() as *mut Handle,
                count,
            )
        };

        handles[0..count].copy_from_slice(wrappers_as_handles);

        unsafe { *size = size_of::<Handle>() * count };

        return Status::SUCCESS;
    }

    Status::UNSUPPORTED
}

pub extern "efiapi" fn locate_device_path(
    _: *mut Guid,
    _: *mut *mut DevicePathProtocol,
    _: *mut *mut c_void,
) -> Status {
    Status::NOT_FOUND
}

pub extern "efiapi" fn install_configuration_table(guid: *mut Guid, table: *mut c_void) -> Status {
    let st = unsafe { &mut ST };
    let ct = unsafe { &mut CT };

    for entry in ct.iter_mut() {
        if entry.vendor_guid == unsafe { *guid } {
            if table.is_null() {
                entry.vendor_guid = INVALID_GUID;
                entry.vendor_table = null_mut();
                st.number_of_table_entries -= 1;
            } else {
                entry.vendor_table = table;
            }
            return Status::SUCCESS;
        }
    }

    if table.is_null() {
        // Trying to delete the table, but not found.
        return Status::NOT_FOUND;
    }

    for entry in ct.iter_mut() {
        if entry.vendor_guid == INVALID_GUID && entry.vendor_table.is_null() {
            entry.vendor_guid = unsafe { *guid };
            entry.vendor_table = table;
            st.number_of_table_entries += 1;
            return Status::SUCCESS;
        }
    }

    Status::OUT_OF_RESOURCES
}

pub extern "efiapi" fn load_image(
    _boot_policy: Boolean,
    parent_image_handle: Handle,
    device_path: *mut DevicePathProtocol,
    _source_buffer: *mut c_void,
    _source_size: usize,
    image_handle: *mut Handle,
) -> Status {
    use crate::fat::Read;

    let device_path = unsafe { &*device_path };
    match &DevicePath::parse(device_path) {
        dp @ DevicePath::File(path) => {
            let path = crate::common::ascii_strip(path);

            let li = parent_image_handle as *const LoadedImageWrapper;
            let dh = unsafe { (*li).proto.device_handle };
            let wrapped_fs_ref = unsafe { &*(dh as *const file::FileSystemWrapper) };
            let mut file = match wrapped_fs_ref.fs.open(path) {
                Ok(file) => file,
                Err(_) => return Status::DEVICE_ERROR,
            };

            let file_size = (file.get_size() as u64 + PAGE_SIZE - 1) / PAGE_SIZE;
            // Get free pages address
            let load_addr =
                match ALLOCATOR
                    .borrow_mut()
                    .find_free_pages(efi::ALLOCATE_ANY_PAGES, file_size, 0)
                {
                    Some(a) => a,
                    None => return Status::OUT_OF_RESOURCES,
                };

            let mut l = crate::pe::Loader::new(&mut file);
            let (entry_addr, load_addr, load_size) = match l.load(load_addr) {
                Ok(load_info) => load_info,
                Err(_) => return Status::DEVICE_ERROR,
            };
            ALLOCATOR.borrow_mut().allocate_pages(
                efi::ALLOCATE_ADDRESS,
                efi::LOADER_CODE,
                file_size,
                load_addr,
            );

            let image = new_image_handle(
                dp.generate(),
                parent_image_handle,
                wrapped_fs_ref as *const _ as Handle,
                load_addr,
                load_size,
                entry_addr,
            );

            unsafe { *image_handle = image as *mut _ as *mut c_void };

            Status::SUCCESS
        }
        _ => Status::UNSUPPORTED,
    }
}

pub extern "efiapi" fn start_image(
    image_handle: Handle,
    _: *mut usize,
    _: *mut *mut Char16,
) -> Status {
    let wrapped_handle = image_handle as *const LoadedImageWrapper;
    let address = unsafe { (*wrapped_handle).entry_point };
    let ptr = address as *const ();
    let code: extern "efiapi" fn(Handle, *mut efi::SystemTable) -> Status =
        unsafe { core::mem::transmute(ptr) };
    (code)(image_handle, unsafe { &mut ST })
}

pub extern "efiapi" fn exit(_: Handle, _: Status, _: usize, _: *mut Char16) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn unload_image(_: Handle) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn exit_boot_services(_: Handle, _: usize) -> Status {
    Status::SUCCESS
}

pub extern "efiapi" fn get_next_monotonic_count(_: *mut u64) -> Status {
    Status::DEVICE_ERROR
}

pub extern "efiapi" fn stall(microseconds: usize) -> Status {
    crate::delay::udelay(microseconds as u64);
    Status::SUCCESS
}

pub extern "efiapi" fn set_watchdog_timer(_: usize, _: u64, _: usize, _: *mut Char16) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn connect_controller(
    _: Handle,
    _: *mut Handle,
    _: *mut DevicePathProtocol,
    _: Boolean,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn disconnect_controller(_: Handle, _: Handle, _: Handle) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn open_protocol(
    handle: Handle,
    guid: *mut Guid,
    out: *mut *mut c_void,
    _: Handle,
    _: Handle,
    _: u32,
) -> Status {
    let hw = handle as *const HandleWrapper;
    let handle_type = unsafe { (*hw).handle_type };
    if unsafe { *guid } == r_efi::protocols::loaded_image::PROTOCOL_GUID
        && handle_type == HandleType::LoadedImage
    {
        unsafe {
            *out = &mut (*(handle as *mut LoadedImageWrapper)).proto as *mut _ as *mut c_void;
        }
        return Status::SUCCESS;
    }

    if unsafe { *guid } == r_efi::protocols::simple_file_system::PROTOCOL_GUID
        && handle_type == HandleType::FileSystem
    {
        unsafe {
            *out = &mut (*(handle as *mut file::FileSystemWrapper)).proto as *mut _ as *mut c_void;
        }
        return Status::SUCCESS;
    }

    if unsafe { *guid } == r_efi::protocols::device_path::PROTOCOL_GUID
        && handle_type == HandleType::Block
    {
        unsafe {
            *out = &mut (*(handle as *mut block::BlockWrapper)).controller_path as *mut _
                as *mut c_void;
        }

        return Status::SUCCESS;
    }

    if unsafe { *guid } == r_efi::protocols::device_path::PROTOCOL_GUID
        && handle_type == HandleType::FileSystem
    {
        unsafe {
            if let Some(block_part_id) = (*(handle as *mut file::FileSystemWrapper)).block_part_id {
                *out = (&mut (*(BLOCK_WRAPPERS.wrappers[block_part_id as usize])).controller_path)
                    as *mut _ as *mut c_void;

                return Status::SUCCESS;
            }
        }
    }

    if unsafe { *guid } == block::PROTOCOL_GUID && handle_type == HandleType::Block {
        unsafe {
            *out = &mut (*(handle as *mut block::BlockWrapper)).proto as *mut _ as *mut c_void;
        }

        return Status::SUCCESS;
    }

    Status::UNSUPPORTED
}

pub extern "efiapi" fn close_protocol(_: Handle, _: *mut Guid, _: Handle, _: Handle) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn open_protocol_information(
    _: Handle,
    _: *mut Guid,
    _: *mut *mut OpenProtocolInformationEntry,
    _: *mut usize,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn protocols_per_handle(
    _: Handle,
    _: *mut *mut *mut Guid,
    _: *mut usize,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn locate_handle_buffer(
    _: LocateSearchType,
    _: *mut Guid,
    _: *mut c_void,
    _: *mut usize,
    _: *mut *mut Handle,
) -> Status {
    Status::UNSUPPORTED
}

#[cfg(target_arch = "riscv64")]
#[repr(C)]
struct RiscVBootProtocol {
    revision: u64,
    get_boot_hart_id: eficall! {fn(*const RiscVBootProtocol, *mut u64) -> Status },
}

#[cfg(target_arch = "riscv64")]
extern "efiapi" fn get_boot_hart_id(_: *const RiscVBootProtocol, hart: *mut u64) -> Status {
    unsafe { *hart = 0 };
    Status::SUCCESS
}

#[cfg(target_arch = "riscv64")]
const RISC_V_BOOT_PROTOCOL: RiscVBootProtocol = RiscVBootProtocol {
    revision: 0,
    get_boot_hart_id,
};

#[cfg(target_arch = "riscv64")]
pub const RISV_V_BOOT_PROTOCOL_GUID: Guid = Guid::from_fields(
    0xccd15fec,
    0x6f73,
    0x4eec,
    0x83,
    0x95,
    &[0x3e, 0x69, 0xe4, 0xb9, 0x40, 0xbf],
);

pub extern "efiapi" fn locate_protocol(
    _guid: *mut Guid,
    _: *mut c_void,
    _out: *mut *mut c_void,
) -> Status {
    #[cfg(target_arch = "riscv64")]
    if unsafe { *_guid } == RISV_V_BOOT_PROTOCOL_GUID {
        unsafe { *_out = &RISC_V_BOOT_PROTOCOL as *const RiscVBootProtocol as *mut c_void };
        return Status::SUCCESS;
    }
    // XXX: A recent version of Linux kernel fails to boot if EFI_UNSUPPORTED returned.
    Status::NOT_FOUND
}

pub extern "efiapi" fn install_multiple_protocol_interfaces(
    _: *mut Handle,
    _: *mut c_void,
    _: *mut c_void,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn uninstall_multiple_protocol_interfaces(
    _: *mut Handle,
    _: *mut c_void,
    _: *mut c_void,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn calculate_crc32(_: *mut c_void, _: usize, _: *mut u32) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn copy_mem(dst: *mut c_void, src: *mut c_void, count: usize) {
    unsafe { core::ptr::copy(src as *const u8, dst as *mut u8, count) }
}

pub extern "efiapi" fn set_mem(dst: *mut c_void, count: usize, val: u8) {
    unsafe { core::ptr::write_bytes(dst as *mut u8, val, count) }
}

pub extern "efiapi" fn create_event_ex(
    _: u32,
    _: Tpl,
    _: Option<EventNotify>,
    _: *const c_void,
    _: *const Guid,
    _: *mut Event,
) -> Status {
    Status::UNSUPPORTED
}

extern "efiapi" fn image_unload(_: Handle) -> Status {
    efi::Status::UNSUPPORTED
}

#[allow(clippy::large_enum_variant)]
enum DevicePath {
    File([u8; 256]),
    Unsupported,
}

impl DevicePath {
    fn parse(dpp: &DevicePathProtocol) -> DevicePath {
        let mut dpp = dpp;
        loop {
            if dpp.r#type == r_efi::protocols::device_path::TYPE_MEDIA && dpp.sub_type == 0x04 {
                let ptr =
                    (dpp as *const _ as u64 + size_of::<DevicePathProtocol>() as u64) as *const u16;
                let mut path = [0u8; 256];
                crate::common::ucs2_to_ascii(ptr, &mut path);
                return DevicePath::File(path);
            }
            if dpp.r#type == r_efi::protocols::device_path::TYPE_END && dpp.sub_type == 0xff {
                log!("Unexpected end of device path");
                return DevicePath::Unsupported;
            }
            let len = unsafe { core::mem::transmute::<[u8; 2], u16>(dpp.length) };
            dpp = unsafe { &*((dpp as *const _ as u64 + len as u64) as *const _) };
        }
    }

    fn generate(&self) -> *mut r_efi::protocols::device_path::Protocol {
        match self {
            Self::File(path) => file_device_path(crate::common::ascii_strip(path)),
            Self::Unsupported => panic!("Cannot generate from unsupported Device Path type"),
        }
    }
}

const PAGE_SIZE: u64 = 4096;
const HEAP_SIZE: usize = 2 << 20; /* 2MiB */

// Populate allocator from E820, fixed ranges for the firmware and the loaded binary.
fn populate_allocator(info: &dyn bootinfo::Info, image_address: u64, image_size: u64) {
    for i in 0..info.num_entries() {
        let entry = info.entry(i);
        match entry.entry_type {
            bootinfo::EntryType::Ram => {
                ALLOCATOR.borrow_mut().add_initial_allocation(
                    efi::CONVENTIONAL_MEMORY,
                    entry.size / PAGE_SIZE,
                    entry.addr,
                    efi::MEMORY_WB,
                );
            }
            _ => continue,
        }
    }

    for descriptor in info.memory_layout() {
        let memory_type = match descriptor.attribute {
            layout::MemoryAttribute::Code => efi::RUNTIME_SERVICES_CODE,
            layout::MemoryAttribute::Data => efi::RUNTIME_SERVICES_DATA,
            layout::MemoryAttribute::Unusable => efi::UNUSABLE_MEMORY,
            layout::MemoryAttribute::Mmio => efi::MEMORY_MAPPED_IO,
        };
        ALLOCATOR.borrow_mut().allocate_pages(
            efi::ALLOCATE_ADDRESS,
            memory_type,
            descriptor.page_count() as u64,
            descriptor.range_start() as u64,
        );
    }

    if let Some(fdt_entry) = info.fdt_reservation() {
        ALLOCATOR.borrow_mut().allocate_pages(
            efi::ALLOCATE_ADDRESS,
            efi::UNUSABLE_MEMORY,
            (fdt_entry.size + 4095) / 4096,
            fdt_entry.addr,
        );
    }

    // Add the loaded binary
    ALLOCATOR.borrow_mut().allocate_pages(
        efi::ALLOCATE_ADDRESS,
        efi::LOADER_CODE,
        image_size / PAGE_SIZE,
        image_address,
    );

    // Initialize heap allocator
    init_heap_allocator(HEAP_SIZE);
}

#[cfg(not(test))]
fn init_heap_allocator(size: usize) {
    let (status, heap_start) = ALLOCATOR.borrow_mut().allocate_pages(
        efi::ALLOCATE_ANY_PAGES,
        efi::BOOT_SERVICES_CODE,
        size as u64 / PAGE_SIZE,
        0,
    );
    assert!(status == Status::SUCCESS);
    unsafe {
        HEAP_ALLOCATOR.lock().init(heap_start as *mut _, size);
    }
}

#[cfg(test)]
fn init_heap_allocator(_: usize) {}

#[repr(C)]
struct LoadedImageWrapper {
    hw: HandleWrapper,
    proto: LoadedImageProtocol,
    entry_point: u64,
}

type DevicePaths = [file::FileDevicePathProtocol; 2];

fn file_device_path(path: &str) -> *mut r_efi::protocols::device_path::Protocol {
    let mut file_paths = null_mut();
    let status = allocate_pool(
        efi::LOADER_DATA,
        size_of::<DevicePaths>(),
        &mut file_paths as *mut *mut c_void,
    );
    assert!(status == Status::SUCCESS);
    let file_paths = unsafe { &mut *(file_paths as *mut DevicePaths) };
    *file_paths = [
        file::FileDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_MEDIA,
                sub_type: 4, // Media Path type file
                length: [(260u16 & 0xff) as u8, (260u16 >> 8) as u8],
            },
            filename: [0; 128],
        },
        file::FileDevicePathProtocol {
            device_path: DevicePathProtocol {
                r#type: r_efi::protocols::device_path::TYPE_END,
                sub_type: 0xff, // End of full path
                length: [4, 0],
            },
            filename: [0; 128],
        },
    ];

    crate::common::ascii_to_ucs2(path, &mut file_paths[0].filename);

    &mut file_paths[0].device_path // Pointer to first path entry
}

fn new_image_handle(
    file_path: *mut r_efi::protocols::device_path::Protocol,
    parent_handle: Handle,
    device_handle: Handle,
    load_addr: u64,
    load_size: u64,
    entry_addr: u64,
) -> *mut LoadedImageWrapper {
    let mut image = null_mut();
    let status = allocate_pool(
        efi::LOADER_DATA,
        size_of::<LoadedImageWrapper>(),
        &mut image as *mut *mut c_void,
    );
    assert!(status == Status::SUCCESS);
    let image = unsafe { &mut *(image as *mut LoadedImageWrapper) };
    *image = LoadedImageWrapper {
        hw: HandleWrapper {
            handle_type: HandleType::LoadedImage,
        },
        proto: LoadedImageProtocol {
            revision: r_efi::protocols::loaded_image::REVISION,
            parent_handle,
            system_table: unsafe { &mut ST },
            device_handle,
            file_path,
            load_options_size: 0,
            load_options: null_mut(),
            image_base: load_addr as *mut _,
            image_size: load_size,
            image_code_type: efi::LOADER_CODE,
            image_data_type: efi::LOADER_DATA,
            unload: image_unload,
            reserved: null_mut(),
        },
        entry_point: entry_addr,
    };
    image
}

pub fn efi_exec(
    address: u64,
    loaded_address: u64,
    loaded_size: u64,
    info: &dyn bootinfo::Info,
    fs: &crate::fat::Filesystem,
    block: *const crate::block::VirtioBlockDevice,
) {
    let vendor_data = 0u32;

    let ct = unsafe { &mut CT };
    let mut ct_index = 0;

    // Populate with FDT table if present
    // To ensure ACPI is used during boot do not include FDT table on aarch64
    // https://github.com/torvalds/linux/blob/d528014517f2b0531862c02865b9d4c908019dc4/arch/arm64/kernel/acpi.c#L203
    #[cfg(not(target_arch = "aarch64"))]
    if let Some(fdt_entry) = info.fdt_reservation() {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0xb1b621d5,
                0xf19c,
                0x41a5,
                0x83,
                0x0b,
                &[0xd9, 0x15, 0x2c, 0x69, 0xaa, 0xe0],
            ),
            vendor_table: fdt_entry.addr as *const u64 as *mut _,
        };
        ct_index += 1;
    }

    // Populate with ACPI RSDP table if present
    if let Some(acpi_rsdp_ptr) = info.rsdp_addr() {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0x8868_e871,
                0xe4f1,
                0x11d3,
                0xbc,
                0x22,
                &[0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
            ),
            vendor_table: acpi_rsdp_ptr as *mut _,
        };
        ct_index += 1;
    }

    // Othwerwise fill with zero vendor data
    if ct_index == 0 {
        ct[ct_index] = efi::ConfigurationTable {
            vendor_guid: Guid::from_fields(
                0x678a_9665,
                0x9957,
                0x4e7c,
                0xa6,
                0x27,
                &[0x34, 0xc9, 0x46, 0x3d, 0xd2, 0xac],
            ),
            vendor_table: &vendor_data as *const _ as *mut _,
        }
    };

    let mut stdin = console::STDIN;
    let mut stdout = console::STDOUT;
    let st = unsafe { &mut ST };
    st.con_in = &mut stdin;
    st.con_out = &mut stdout;
    st.std_err = &mut stdout;
    st.runtime_services = unsafe { &mut RS };
    st.boot_services = unsafe { &mut BS };
    st.number_of_table_entries = 1;
    st.configuration_table = &mut ct[0];

    populate_allocator(info, loaded_address, loaded_size);

    let efi_part_id = unsafe { block::populate_block_wrappers(&mut BLOCK_WRAPPERS, block) };

    let wrapped_fs = file::FileSystemWrapper::new(fs, efi_part_id);

    let mut path = [0u8; 256];
    path[0..crate::efi::EFI_BOOT_PATH.as_bytes().len()]
        .copy_from_slice(crate::efi::EFI_BOOT_PATH.as_bytes());
    let device_path = DevicePath::File(path);
    let image = new_image_handle(
        device_path.generate(),
        0 as Handle,
        &wrapped_fs as *const _ as Handle,
        loaded_address,
        loaded_size,
        address,
    );

    let ptr = address as *const ();
    let code: extern "efiapi" fn(Handle, *mut efi::SystemTable) -> Status =
        unsafe { core::mem::transmute(ptr) };
    (code)((image as *const _) as Handle, &mut *st);
}
