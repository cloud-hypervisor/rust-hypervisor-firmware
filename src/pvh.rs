use crate::{
    boot::{E820Entry, Info},
    common,
};

// Structures from xen/include/public/arch-x86/hvm/start_info.h
#[derive(Debug)]
#[repr(C)]
pub struct StartInfo {
    magic: [u8; 4],
    version: u32,
    flags: u32,
    nr_modules: u32,
    modlist_paddr: u64,
    cmdline_paddr: u64,
    rsdp_paddr: u64,
    memmap_paddr: u64,
    memmap_entries: u32,
    _pad: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct MemMapEntry {
    addr: u64,
    size: u64,
    entry_type: u32,
    _pad: u32,
}

impl Info for StartInfo {
    fn rsdp_addr(&self) -> u64 {
        self.rsdp_paddr
    }
    fn cmdline(&self) -> &[u8] {
        unsafe { common::from_cstring(self.cmdline_paddr) }
    }
    fn num_entries(&self) -> u8 {
        // memmap_paddr and memmap_entries only exist in version 1 or later
        if self.version < 1 || self.memmap_paddr == 0 {
            return 0;
        }
        self.memmap_entries as u8
    }
    fn entry(&self, idx: u8) -> E820Entry {
        assert!(idx < self.num_entries());
        let ptr = self.memmap_paddr as *const MemMapEntry;
        let entry = unsafe { *ptr.offset(idx as isize) };
        E820Entry {
            addr: entry.addr,
            size: entry.size,
            entry_type: entry.entry_type,
        }
    }
}
