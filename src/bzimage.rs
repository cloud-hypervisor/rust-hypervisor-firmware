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

use crate::fat;
use fat::Read;

#[cfg(not(test))]
pub enum Error {
    FileError,
    KernelOld,
    MagicMissing,
    NotRelocatable,
}

#[cfg(not(test))]
impl From<fat::Error> for Error {
    fn from(_: fat::Error) -> Error {
        Error::FileError
    }
}

// From firecracker
#[cfg(not(test))]
/// Kernel command line start address.
const CMDLINE_START: usize = 0x4b000;
#[cfg(not(test))]
/// Kernel command line start address maximum size.
const CMDLINE_MAX_SIZE: usize = 0x10000;
#[cfg(not(test))]
/// The 'zero page', a.k.a linux kernel bootparams.
pub const ZERO_PAGE_START: usize = 0x7000;

#[cfg(not(test))]
const KERNEL_LOCATION: u32 = 0x20_0000;

#[cfg(not(test))]
const E820_RAM: u32 = 1;

#[cfg(not(test))]
#[repr(C, packed)]
struct E820Entry {
    addr: u64,
    size: u64,
    entry_type: u32,
}

#[cfg(not(test))]
pub fn load_initrd(f: &mut Read) -> Result<(), Error> {
    let mut zero_page = crate::mem::MemoryRegion::new(ZERO_PAGE_START as u64, 4096);

    let mut max_load_address = u64::from(zero_page.read_u32(0x22c));
    if max_load_address == 0 {
        max_load_address = 0x37ff_ffff;
    }

    let e820_count = zero_page.read_u8(0x1e8);
    let e820_table = zero_page.as_mut_slice::<E820Entry>(0x2d0, u64::from(e820_count));

    // Search E820 table for highest usable ram location that is below the limit.
    let mut top_of_usable_ram = 0;
    for entry in e820_table {
        if entry.entry_type == E820_RAM {
            let m = entry.addr + entry.size - 1;
            if m > top_of_usable_ram && m < max_load_address {
                top_of_usable_ram = m;
            }
        }
    }

    if top_of_usable_ram > max_load_address {
        top_of_usable_ram = max_load_address;
    }

    let initrd_address = top_of_usable_ram - u64::from(f.get_size());
    let mut initrd_region = crate::mem::MemoryRegion::new(initrd_address, u64::from(f.get_size()));

    let mut offset = 0;
    while offset < f.get_size() {
        let bytes_remaining = f.get_size() - offset;

        // Use intermediata buffer for last, partial sector
        if bytes_remaining < 512 {
            let mut data: [u8; 512] = [0; 512];
            match f.read(&mut data) {
                Err(crate::fat::Error::EndOfFile) => break,
                Err(_) => return Err(Error::FileError),
                Ok(_) => {}
            }
            let dst = initrd_region.as_mut_slice(u64::from(offset), u64::from(bytes_remaining));
            dst.copy_from_slice(&data[0..bytes_remaining as usize]);
            break;
        }

        let dst = initrd_region.as_mut_slice(u64::from(offset), 512);

        match f.read(dst) {
            Err(crate::fat::Error::EndOfFile) => break,
            Err(_) => return Err(Error::FileError),
            Ok(_) => {}
        }

        offset += 512;
    }

    // initrd pointer/size
    zero_page.write_u32(0x218, initrd_address as u32);
    zero_page.write_u32(0x21c, f.get_size());
    Ok(())
}

#[cfg(not(test))]
pub fn append_commandline(addition: &str) -> Result<(), Error> {
    let mut cmdline_region =
        crate::mem::MemoryRegion::new(CMDLINE_START as u64, CMDLINE_MAX_SIZE as u64);
    let zero_page = crate::mem::MemoryRegion::new(ZERO_PAGE_START as u64, 4096);

    let cmdline = cmdline_region.as_mut_slice::<u8>(0, CMDLINE_MAX_SIZE as u64);

    // Use the actual string length but limit to the orgiginal incoming size
    let orig_len = zero_page.read_u32(0x238) as usize;

    let orig_cmdline = unsafe {
        core::str::from_utf8_unchecked(&cmdline[0..orig_len]).trim_matches(char::from(0))
    };
    let orig_len = orig_cmdline.len();

    cmdline[orig_len] = b' ';
    cmdline[orig_len + 1..orig_len + 1 + addition.len()].copy_from_slice(addition.as_bytes());
    cmdline[orig_len + 1 + addition.len()] = 0;

    // Commandline pointer/size
    zero_page.write_u32(0x228, CMDLINE_START as u32);
    zero_page.write_u32(0x238, (orig_len + addition.len() + 1) as u32);

    Ok(())
}

#[cfg(not(test))]
pub fn load_kernel(f: &mut Read) -> Result<(u64), Error> {
    f.seek(0)?;

    let mut buf: [u8; 1024] = [0; 1024];

    f.read(&mut buf[0..512])?;
    f.read(&mut buf[512..])?;

    let setup = crate::mem::MemoryRegion::from_slice(&buf[..]);

    if setup.read_u16(0x1fe) != 0xAA55 {
        return Err(Error::MagicMissing);
    }

    if setup.read_u32(0x202) != 0x5372_6448 {
        return Err(Error::MagicMissing);
    }

    // Need for relocation
    if setup.read_u16(0x206) < 0x205 {
        return Err(Error::KernelOld);
    }

    // Check relocatable
    if setup.read_u8(0x234) == 0 {
        return Err(Error::NotRelocatable);
    }

    let header_start = 0x1f1 as usize;
    let header_end = 0x202 + buf[0x0201] as usize;

    // Reuse the zero page that we were originally given
    // TODO: Zero and fill it ourself but will need to save E820 details
    let mut zero_page = crate::mem::MemoryRegion::new(ZERO_PAGE_START as u64, 4096);

    let dst = zero_page.as_mut_slice(header_start as u64, (header_end - header_start) as u64);
    dst.copy_from_slice(&buf[header_start..header_end]);

    // Unknown loader
    zero_page.write_u8(0x210, 0xff);

    // Where we will load the kernel into
    zero_page.write_u32(0x214, KERNEL_LOCATION);

    let mut setup_sects = buf[header_start] as usize;

    if setup_sects == 0 {
        setup_sects = 4;
    }

    setup_sects += 1; // Include the boot sector

    let setup_bytes = setup_sects * 512; // Use to start reading the main image

    let mut load_offset = u64::from(KERNEL_LOCATION);

    f.seek(setup_bytes as u32)?;

    loop {
        let mut dst = crate::mem::MemoryRegion::new(load_offset, 512);
        let dst = dst.as_mut_slice(0, 512);

        match f.read(dst) {
            Err(crate::fat::Error::EndOfFile) => {
                // 0x200 is the startup_64 offset
                return Ok(u64::from(KERNEL_LOCATION) + 0x200);
            }
            Err(_) => return Err(Error::FileError),
            Ok(_) => {}
        };

        load_offset += 512;
    }
}
