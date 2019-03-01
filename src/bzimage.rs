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

use crate::fat::Read;

#[cfg(not(test))]
pub enum Error {
    FileError,
    KernelOld,
    MagicMissing,
    NotRelocatable,
}

// From firecracker
#[cfg(not(test))]
/// Kernel command line start address.
const CMDLINE_START: usize = 0x20000;
#[cfg(not(test))]
/// Kernel command line start address maximum size.
const CMDLINE_MAX_SIZE: usize = 0x10000;
#[cfg(not(test))]
/// The 'zero page', a.k.a linux kernel bootparams.
pub const ZERO_PAGE_START: usize = 0x7000;

#[cfg(not(test))]
const KERNEL_LOCATION: u32 = 0x200000;

#[cfg(not(test))]
pub fn load_commandline(f: &mut Read) -> Result<(), Error> {
    let cmdline_region =
        crate::mem::MemoryRegion::new(CMDLINE_START as u64, CMDLINE_MAX_SIZE as u64);

    let dst = cmdline_region.as_mut_slice::<u8>(0, CMDLINE_MAX_SIZE as u64);
    for x in 0..dst.len() {
        dst[x] = 0;
    }

    let mut offset = 0;
    while offset < CMDLINE_MAX_SIZE {
        // There is no need to worry about partial sectors here as the mapped range
        // is a multiple of the sector size.
        let dst = cmdline_region.as_mut_slice(offset as u64, 512);

        match f.read(dst) {
            Err(crate::fat::Error::EndOfFile) => break,
            Err(_) => return Err(Error::FileError),
            Ok(_) => {}
        }

        offset += 512;
    }

    // We can do this safely and (the .len()) later as we zero all the range.
    let cmdline = unsafe { core::str::from_utf8_unchecked(dst) };

    let zero_page = crate::mem::MemoryRegion::new(ZERO_PAGE_START as u64, 4096);

    // Commandline pointer/size
    zero_page.write_u32(0x228, CMDLINE_START as u32);
    zero_page.write_u32(0x238, cmdline.len() as u32);

    Ok(())
}

#[cfg(not(test))]
pub fn load_kernel(f: &mut Read) -> Result<(u64), Error> {
    match f.seek(0) {
        Err(_) => return Err(Error::FileError),
        Ok(_) => {}
    };

    let mut buf: [u8; 1024] = [0; 1024];

    match f.read(&mut buf[0..512]) {
        Err(_) => return Err(Error::FileError),
        Ok(_) => {}
    };

    match f.read(&mut buf[512..]) {
        Err(_) => return Err(Error::FileError),
        Ok(_) => {}
    };

    let setup = crate::mem::MemoryRegion::from_slice(&buf[..]);

    if setup.read_u16(0x1fe) != 0xAA55 {
        return Err(Error::MagicMissing);
    }

    if setup.read_u32(0x202) != 0x53726448 {
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
    let zero_page = crate::mem::MemoryRegion::new(ZERO_PAGE_START as u64, 4096);

    let dst = zero_page.as_mut_slice(header_start as u64, (header_end - header_start) as u64);
    dst.copy_from_slice(&buf[header_start..header_end]);

    // Populate command line memory
    let cmdline = "console=ttyS0";
    let cmdline_region =
        crate::mem::MemoryRegion::new(CMDLINE_START as u64, CMDLINE_MAX_SIZE as u64);

    let dst = cmdline_region.as_mut_slice(0, CMDLINE_MAX_SIZE as u64);
    for x in 0..dst.len() {
        dst[x] = 0;
    }

    let dst = &mut dst[0..cmdline.len()];
    dst.copy_from_slice(cmdline.as_bytes());

    // Unknown loader
    zero_page.write_u8(0x210, 0xff);

    // Commandline pointer/size
    zero_page.write_u32(0x228, CMDLINE_START as u32);
    zero_page.write_u32(0x238, cmdline.len() as u32);

    // Where we will load the kernel into
    zero_page.write_u32(0x214, KERNEL_LOCATION);

    let mut setup_sects = buf[header_start] as usize;

    if setup_sects == 0 {
        setup_sects = 4;
    }

    setup_sects += 1; // Include the boot sector

    let setup_bytes = setup_sects * 512; // Use to start reading the main image

    let mut load_offset = KERNEL_LOCATION as u64;

    match f.seek(setup_bytes as u32) {
        Err(_) => return Err(Error::FileError),
        Ok(_) => {}
    };

    loop {
        let dst = crate::mem::MemoryRegion::new(load_offset, 512);
        let dst = dst.as_mut_slice(0, 512);

        match f.read(dst) {
            Err(crate::fat::Error::EndOfFile) => {
                // 0x200 is the startup_64 offset
                return Ok(KERNEL_LOCATION as u64 + 0x200);
            }
            Err(_) => return Err(Error::FileError),
            Ok(_) => {}
        };

        load_offset += 512;
    }
}
