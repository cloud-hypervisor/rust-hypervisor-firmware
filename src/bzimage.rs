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
use atomic_refcell::AtomicRefCell;

use crate::{
    block::SectorBuf,
    boot::{E820Entry, Header, Info, Params},
    fat::{self, Read},
    mem::MemoryRegion,
};

#[derive(Debug)]
pub enum Error {
    File(fat::Error),
    NoInitrdMemory,
    MagicMissing,
    NotRelocatable,
}

impl From<fat::Error> for Error {
    fn from(e: fat::Error) -> Error {
        Error::File(e)
    }
}

const KERNEL_LOCATION: u64 = 0x20_0000;

#[repr(transparent)]
pub struct Kernel(Params);

impl Kernel {
    pub fn new(info: &dyn Info) -> Self {
        let mut kernel = Self(Params::default());
        kernel.0.acpi_rsdp_addr = info.rsdp_addr();
        kernel.0.set_entries(info);
        kernel
    }

    pub fn load_kernel(&mut self, f: &mut dyn Read) -> Result<(), Error> {
        self.0.hdr = Header::from_file(f)?;

        if self.0.hdr.boot_flag != 0xAA55 || self.0.hdr.header != *b"HdrS" {
            return Err(Error::MagicMissing);
        }
        // Check relocatable
        if self.0.hdr.version < 0x205 || self.0.hdr.relocatable_kernel == 0 {
            return Err(Error::NotRelocatable);
        }

        // Skip over the setup sectors
        let setup_sects = match self.0.hdr.setup_sects {
            0 => 4,
            n => n as u32,
        };
        let setup_bytes = (setup_sects + 1) * SectorBuf::len() as u32;
        let remaining_bytes = f.get_size() - setup_bytes;

        let mut region = MemoryRegion::new(KERNEL_LOCATION, remaining_bytes as u64);
        f.seek(setup_bytes)?;
        f.load_file(&mut region)?;

        // Fill out "write/modify" fields
        self.0.hdr.type_of_loader = 0xff; // Unknown Loader
        self.0.hdr.code32_start = KERNEL_LOCATION as u32; // Where we load the kernel
        self.0.hdr.cmd_line_ptr = CMDLINE_START as u32; // Where we load the cmdline
        Ok(())
    }

    // Compute the load address for the initial ramdisk
    fn initrd_addr(&self, size: u64) -> Option<u64> {
        let initrd_addr_max = match self.0.hdr.initrd_addr_max {
            0 => 0x37FF_FFFF,
            a => a as u64,
        };

        // Limit to 4GiB identity mapped area
        let initrd_addr_max = u64::min(initrd_addr_max, (4 << 30) - 1);

        let max_start = (initrd_addr_max + 1) - size;

        // Align address to 2MiB boundary as we use 2 MiB pages
        let max_start = max_start & !((2 << 20) - 1);

        let mut current_addr = None;
        for i in 0..self.0.num_entries() {
            let entry = self.0.entry(i);
            if entry.entry_type != E820Entry::RAM_TYPE {
                continue;
            }

            // Disregard regions beyond the max
            if entry.addr > max_start {
                continue;
            }

            // Disregard regions that are too small
            if size > entry.size {
                continue;
            }

            // Place at the top of the region
            let potential_addr = entry.addr + entry.size - size;

            // Align address to 2MiB boundary as we use 2 MiB pages
            let potential_addr = potential_addr & !((2 << 20) - 1);

            // But clamp to the maximum start
            let potential_addr = u64::min(potential_addr, max_start);

            // Use the higest address we can find
            if let Some(current_addr) = current_addr {
                if current_addr >= potential_addr {
                    continue;
                }
            }
            current_addr = Some(potential_addr)
        }
        current_addr
    }

    pub fn load_initrd(&mut self, f: &mut dyn Read) -> Result<(), Error> {
        let size = f.get_size() as u64;
        let addr = match self.initrd_addr(size) {
            Some(addr) => addr,
            None => return Err(Error::NoInitrdMemory),
        };

        let mut region = MemoryRegion::new(addr, size);
        f.seek(0)?;
        f.load_file(&mut region)?;

        // initrd pointer/size
        self.0.hdr.ramdisk_image = addr as u32;
        self.0.hdr.ramdisk_size = size as u32;
        Ok(())
    }

    pub fn append_cmdline(&mut self, addition: &[u8]) {
        if !addition.is_empty() {
            CMDLINE.borrow_mut().append(addition);
            assert!(CMDLINE.borrow().len() < self.0.hdr.cmdline_size);
        }
    }

    pub fn boot(&mut self) {
        // 0x200 is the startup_64 offset
        let jump_address = self.0.hdr.code32_start as u64 + 0x200;
        // Rely on x86 C calling convention where second argument is put into %rsi register
        let ptr = jump_address as *const ();
        let code: extern "C" fn(usize, usize) = unsafe { core::mem::transmute(ptr) };
        (code)(0 /* dummy value */, &mut self.0 as *mut _ as usize);
    }
}

// This is the highest region at which we can load the kernel command line.
const CMDLINE_START: u64 = 0x4b000;
const CMDLINE_MAX_LEN: u64 = 0x10000;

static CMDLINE: AtomicRefCell<CmdLine> = AtomicRefCell::new(CmdLine::new());

struct CmdLine {
    region: MemoryRegion,
    length: usize, // Does not include null pointer
}

impl CmdLine {
    const fn new() -> Self {
        Self {
            region: MemoryRegion::new(CMDLINE_START, CMDLINE_MAX_LEN),
            length: 0,
        }
    }

    const fn len(&self) -> u32 {
        self.length as u32
    }

    fn append(&mut self, args: &[u8]) {
        let bytes = self.region.as_bytes();
        bytes[self.length] = b' ';
        self.length += 1;

        bytes[self.length..self.length + args.len()].copy_from_slice(args);
        self.length += args.len();
        bytes[self.length] = 0;
    }
}
