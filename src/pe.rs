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

use crate::{block::SectorBuf, mem::MemoryRegion};

pub struct Loader<'a> {
    file: &'a mut dyn crate::fat::Read,
    num_sections: u16,
    image_base: u64,
    image_size: u32,
}

#[derive(Debug)]
pub enum Error {
    FileError,
    InvalidExecutable,
}

#[repr(packed)]
struct Section {
    _name: [u8; 8],
    virt_size: u32,
    virt_address: u32,
    raw_size: u32,
    raw_offset: u32,
    _unused: [u8; 16],
}

impl<'a> Loader<'a> {
    #[cfg(target_arch = "aarch64")]
    const MACHINE_TYPE: u16 = 0xaa64;
    #[cfg(target_arch = "x86_64")]
    const MACHINE_TYPE: u16 = 0x8664;

    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    const OPTIONAL_HEADER_MAGIC: u16 = 0x20b; // PE32+

    pub fn new(file: &'a mut dyn crate::fat::Read) -> Loader {
        Loader {
            file,
            num_sections: 0,
            image_base: 0,
            image_size: 0,
        }
    }

    pub fn load(&mut self, load_addr: u64) -> Result<(u64, u64, u64), Error> {
        const HEADER_SIZE: usize = 1024;
        let mut data = [0_u8; HEADER_SIZE];
        assert!(data.len() % 512 == 0);

        let mut buf = SectorBuf::new();
        match self.file.read(buf.as_mut_bytes()) {
            Ok(_) => {}
            Err(_) => return Err(Error::FileError),
        }
        let sector_size = SectorBuf::len();
        if data.len() <= sector_size {
            data.copy_from_slice(&buf.as_bytes()[..HEADER_SIZE]);
        } else if data.len() == sector_size * 2 {
            // The sector size is 512
            data[..sector_size].copy_from_slice(buf.as_bytes());
            match self.file.read(buf.as_mut_bytes()) {
                Ok(_) => {
                    data[sector_size..].copy_from_slice(buf.as_bytes());
                }
                Err(_) => return Err(Error::FileError),
            }
        } else {
            // Unsupported sector size
            return Err(Error::FileError);
        }

        let dos_region = MemoryRegion::from_bytes(&mut data);

        // 'MZ' magic
        if dos_region.read_u16(0) != 0x5a4d {
            return Err(Error::InvalidExecutable);
        }

        // offset to COFF header
        let pe_header_offset = dos_region.read_u32(0x3c);

        if pe_header_offset >= sector_size as u32 {
            return Err(Error::InvalidExecutable);
        }

        let pe_region = MemoryRegion::from_bytes(&mut data[pe_header_offset as usize..]);

        // The Microsoft specification uses offsets relative to the COFF area
        // which is 4 after the signature (so all offsets are +4 relative to the spec)
        // 'PE' magic
        if pe_region.read_u32(0) != 0x0000_4550 {
            return Err(Error::InvalidExecutable);
        }

        // Check for supported machine
        if pe_region.read_u16(4) != Self::MACHINE_TYPE {
            return Err(Error::InvalidExecutable);
        }

        self.num_sections = pe_region.read_u16(6);

        let optional_header_size = pe_region.read_u16(20);
        let optional_region =
            MemoryRegion::from_bytes(&mut data[(24 + pe_header_offset) as usize..]);

        if optional_region.read_u16(0) != Self::OPTIONAL_HEADER_MAGIC {
            return Err(Error::InvalidExecutable);
        }

        let entry_point = optional_region.read_u32(16);

        self.image_base = optional_region.read_u64(24);
        let address = if self.image_base != 0 {
            // The image has desired load address
            self.image_base
        } else {
            load_addr
        };
        self.image_size = optional_region.read_u32(56);
        let size_of_headers = optional_region.read_u32(60);

        let sections = &data[(24 + pe_header_offset + u32::from(optional_header_size)) as usize..];
        let sections: &[Section] = unsafe {
            core::slice::from_raw_parts(
                sections.as_ptr() as *const Section,
                self.num_sections as usize,
            )
        };

        let image_info = (
            address + u64::from(entry_point),
            address,
            u64::from(self.image_size),
        );

        let mut loaded_region = MemoryRegion::new(address, u64::from(self.image_size));

        // Copy the PE header into the start of the destination memory
        match self.file.seek(0) {
            Ok(_) => {}
            Err(_) => return Err(Error::FileError),
        }

        let mut header_offset = 0u64;
        while header_offset < u64::from(size_of_headers) {
            match self
                .file
                .read(loaded_region.as_mut_slice(header_offset, sector_size as u64))
            {
                Ok(_) => {}
                Err(_) => {
                    return Err(Error::FileError);
                }
            }
            header_offset += sector_size as u64;
        }

        for section in sections {
            for x in 0..section.virt_size {
                loaded_region.write_u8(u64::from(x) + u64::from(section.virt_address), 0);
            }

            // TODO: Handle strange offset sections.
            if section.raw_offset % sector_size as u32 != 0 {
                continue;
            }

            match self.file.seek(section.raw_offset) {
                Ok(_) => {}
                Err(_) => return Err(Error::FileError),
            }

            let mut section_data = SectorBuf::new();

            let mut section_offset = 0;
            let section_size = core::cmp::min(section.raw_size, section.virt_size);
            while section_offset < section_size {
                let remaining_bytes =
                    core::cmp::min(section_size - section_offset, sector_size as u32);
                match self.file.read(section_data.as_mut_bytes()) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(Error::FileError);
                    }
                }

                let l: &mut [u8] = loaded_region.as_mut_slice(
                    u64::from(section.virt_address + section_offset),
                    u64::from(remaining_bytes),
                );
                l.copy_from_slice(&section_data.as_bytes()[0..remaining_bytes as usize]);
                section_offset += remaining_bytes;
            }
        }

        let base_diff = address as i64 - self.image_base as i64;

        let num_data_dirs = optional_region.read_u32(108);
        if num_data_dirs < 5 {
            // No base relocation table entry
            return Ok(image_info);
        }
        let reloc_dir_virt_addr = optional_region.read_u32(152);
        let reloc_dir_size = optional_region.read_u32(156);
        if reloc_dir_virt_addr == 0 || reloc_dir_size == 0 {
            // No base relocation table available
            return Ok(image_info);
        }
        for section in sections {
            if section.virt_address == reloc_dir_virt_addr
                && section.raw_offset % sector_size as u32 != 0
            {
                // This section is not loaded
                return Ok(image_info);
            }
        }

        let section_size = reloc_dir_size;
        let l: &mut [u8] =
            loaded_region.as_mut_slice(u64::from(reloc_dir_virt_addr), u64::from(section_size));

        let reloc_region = MemoryRegion::from_bytes(l);

        let mut section_bytes_remaining = section_size;
        let mut offset = 0;
        while section_bytes_remaining > 0 {
            // Read details for block
            let page_rva = reloc_region.read_u32(offset);
            let block_size = reloc_region.read_u32(offset + 4);
            let mut block_offset = 8;
            while block_offset < block_size {
                let entry = reloc_region.read_u16(offset + u64::from(block_offset));

                let entry_type = entry >> 12;
                let entry_offset = entry & 0xfff;

                if entry_type == 10 {
                    let location = u64::from(page_rva + u32::from(entry_offset));
                    let value = loaded_region.read_u64(location);
                    loaded_region.write_u64(location, (value as i64 + base_diff) as u64);
                }

                block_offset += 2;
            }

            section_bytes_remaining -= block_size;
            offset += u64::from(block_size);
        }

        Ok(image_info)
    }
}

#[cfg(test)]
mod tests {
    use crate::part::tests::*;

    use std::alloc;

    // TODO: Add aarch64 specific loader test target
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_loader() {
        let d = FakeDisk::new(&clear_disk_path());
        let (start, end) = crate::part::find_efi_partition(&d).unwrap();

        let mut f = crate::fat::Filesystem::new(&d, start, end);
        f.init().unwrap();
        let mut file = f.open("/EFI/BOOT/BOOTX64 EFI").unwrap();
        let mut l = super::Loader::new(&mut file);

        let fake_mem = unsafe {
            let layout = alloc::Layout::from_size_align(64 * 1024 * 1024, 1024 * 1024).unwrap();
            alloc::alloc(layout)
        };

        let (entry, addr, size) = l.load(fake_mem as u64).expect("expect loading success");
        assert_eq!(entry, fake_mem as u64 + 0x4000);
        assert_eq!(addr, fake_mem as u64);
        assert_eq!(size, 110_592);
    }
}
