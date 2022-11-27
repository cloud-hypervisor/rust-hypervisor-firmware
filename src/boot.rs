// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use core::mem;

use crate::{
    bootinfo::{EntryType, Info, MemoryEntry},
    common,
    fat::{Error, Read},
    mem::MemoryRegion,
};

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub entry_type: u32,
}

impl E820Entry {
    pub const RAM_TYPE: u32 = 1;
    pub const RESERVED_TYPE: u32 = 2;
    pub const ACPI_RECLAIMABLE_TYPE: u32 = 3;
    pub const ACPI_NVS_TYPE: u32 = 4;
    pub const BAD_TYPE: u32 = 5;
    pub const VENDOR_RESERVED_TYPE: u32 = 6; // coreboot only
    pub const COREBOOT_TABLE_TYPE: u32 = 16; // coreboot only
}

impl From<u32> for EntryType {
    fn from(value: u32) -> Self {
        match value {
            E820Entry::RAM_TYPE => Self::Ram,
            E820Entry::RESERVED_TYPE => Self::Reserved,
            E820Entry::ACPI_RECLAIMABLE_TYPE => Self::AcpiReclaimable,
            E820Entry::ACPI_NVS_TYPE => Self::AcpiNvs,
            E820Entry::BAD_TYPE => Self::Bad,
            E820Entry::VENDOR_RESERVED_TYPE => Self::VendorReserved,
            E820Entry::COREBOOT_TABLE_TYPE => Self::CorebootTable,
            _ => panic!("Unsupported e820 type"),
        }
    }
}

impl From<EntryType> for u32 {
    fn from(value: EntryType) -> Self {
        match value {
            EntryType::Ram => E820Entry::RAM_TYPE,
            EntryType::Reserved => E820Entry::RESERVED_TYPE,
            EntryType::AcpiReclaimable => E820Entry::ACPI_RECLAIMABLE_TYPE,
            EntryType::AcpiNvs => E820Entry::ACPI_NVS_TYPE,
            EntryType::Bad => E820Entry::BAD_TYPE,
            EntryType::VendorReserved => E820Entry::VENDOR_RESERVED_TYPE,
            EntryType::CorebootTable => E820Entry::COREBOOT_TABLE_TYPE,
        }
    }
}

impl From<MemoryEntry> for E820Entry {
    fn from(value: MemoryEntry) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            entry_type: u32::from(value.entry_type),
        }
    }
}

impl From<E820Entry> for MemoryEntry {
    fn from(value: E820Entry) -> Self {
        Self {
            addr: value.addr,
            size: value.size,
            entry_type: EntryType::from(value.entry_type),
        }
    }
}

// The so-called "zeropage"
#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Params {
    screen_info: ScreenInfo,        // 0x000
    apm_bios_info: ApmBiosInfo,     // 0x040
    _pad2: [u8; 4],                 // 0x054
    tboot_addr: u64,                // 0x058
    ist_info: IstInfo,              // 0x060
    pub acpi_rsdp_addr: u64,        // 0x070
    _pad3: [u8; 8],                 // 0x078
    hd0_info: HdInfo,               // 0x080 - obsolete
    hd1_info: HdInfo,               // 0x090 - obsolete
    sys_desc_table: SysDescTable,   // 0x0a0 - obsolete
    olpc_ofw_header: OlpcOfwHeader, // 0x0b0
    ext_ramdisk_image: u32,         // 0x0c0
    ext_ramdisk_size: u32,          // 0x0c4
    ext_cmd_line_ptr: u32,          // 0x0c8
    _pad4: [u8; 0x74],              // 0x0cc
    edd_info: EdidInfo,             // 0x140
    efi_info: EfiInfo,              // 0x1c0
    alt_mem_k: u32,                 // 0x1e0
    scratch: u32,                   // 0x1e4
    e820_entries: u8,               // 0x1e8
    eddbuf_entries: u8,             // 0x1e9
    edd_mbr_sig_buf_entries: u8,    // 0x1ea
    kbd_status: u8,                 // 0x1eb
    secure_boot: u8,                // 0x1ec
    _pad5: [u8; 2],                 // 0x1ed
    sentinel: u8,                   // 0x1ef
    _pad6: [u8; 1],                 // 0x1f0
    pub hdr: Header,                // 0x1f1
    _pad7: [u8; 0x290 - HEADER_END],
    edd_mbr_sig_buffer: [u32; 16], // 0x290
    e820_table: [E820Entry; 128],  // 0x2d0
    _pad8: [u8; 0x30],             // 0xcd0
    eddbuf: [EddInfo; 6],          // 0xd00
    _pad9: [u8; 0x114],            // 0xeec
}

impl Default for Params {
    fn default() -> Self {
        // SAFETY: Struct consists entirely of primitive integral types.
        unsafe { mem::zeroed() }
    }
}

impl Params {
    pub fn set_entries(&mut self, info: &dyn Info) {
        self.e820_entries = info.num_entries() as u8;
        for i in 0..self.e820_entries {
            self.e820_table[i as usize] = info.entry(i as usize).into();
        }
    }
}

impl Info for Params {
    fn name(&self) -> &str {
        "Linux Boot Protocol"
    }
    fn rsdp_addr(&self) -> u64 {
        self.acpi_rsdp_addr
    }
    fn cmdline(&self) -> &[u8] {
        unsafe { common::from_cstring(self.hdr.cmd_line_ptr as u64) }
    }
    fn num_entries(&self) -> usize {
        self.e820_entries as usize
    }
    fn entry(&self, idx: usize) -> MemoryEntry {
        assert!(idx < self.num_entries());
        let entry = self.e820_table[idx];
        MemoryEntry::from(entry)
    }
}

const HEADER_START: usize = 0x1f1;
const HEADER_END: usize = HEADER_START + mem::size_of::<Header>();

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Header {
    pub setup_sects: u8,
    pub root_flags: u16,
    pub syssize: u32,
    pub ram_size: u16,
    pub vid_mode: u16,
    pub root_dev: u16,
    pub boot_flag: u16,
    pub jump: u16,
    pub header: [u8; 4],
    pub version: u16,
    pub realmode_swtch: u32,
    pub start_sys_seg: u16,
    pub kernel_version: u16,
    pub type_of_loader: u8,
    pub loadflags: u8,
    pub setup_move_size: u16,
    pub code32_start: u32,
    pub ramdisk_image: u32,
    pub ramdisk_size: u32,
    pub bootsect_kludge: u32,
    pub heap_end_ptr: u16,
    pub ext_loader_ver: u8,
    pub ext_loader_type: u8,
    pub cmd_line_ptr: u32,
    pub initrd_addr_max: u32,
    pub kernel_alignment: u32,
    pub relocatable_kernel: u8,
    pub min_alignment: u8,
    pub xloadflags: u16,
    pub cmdline_size: u32,
    pub hardware_subarch: u32,
    pub hardware_subarch_data: u64,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub setup_data: u64,
    pub pref_address: u64,
    pub init_size: u32,
    pub handover_offset: u32,
}

impl Header {
    // Read a kernel header from the first two sectors of a file
    pub fn from_file(f: &mut dyn Read) -> Result<Self, Error> {
        let mut data: [u8; 1024] = [0; 1024];
        let mut region = MemoryRegion::from_bytes(&mut data);

        f.seek(0)?;
        f.load_file(&mut region)?;

        #[repr(C)]
        struct HeaderData {
            before: [u8; HEADER_START],
            hdr: Header,
            after: [u8; 1024 - HEADER_END],
        }
        // SAFETY: Struct consists entirely of primitive integral types.
        Ok(unsafe { mem::transmute::<_, HeaderData>(data) }.hdr)
    }
}

// Right now the stucts below are unused, so we only need them to be the correct
// size. Update test_size_and_offset if a struct's real definition is added.
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct ScreenInfo([u8; 0x40]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct ApmBiosInfo([u8; 0x14]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IstInfo([u8; 0x10]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct HdInfo([u8; 0x10]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct SysDescTable([u8; 0x10]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct OlpcOfwHeader([u8; 0x10]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct EdidInfo([u8; 0x80]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct EfiInfo([u8; 0x20]);
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct EddInfo([u8; 0x52]);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_size_and_offset() {
        assert_eq!(mem::size_of::<Header>(), 119);
        assert_eq!(mem::size_of::<E820Entry>(), 20);
        assert_eq!(mem::size_of::<Params>(), 4096);

        assert_eq!(offset_of!(Params, hdr), HEADER_START);
    }
}
