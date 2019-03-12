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

use crate::mem::MemoryRegion;

pub struct Loader<'a> {
    file: &'a mut crate::fat::Read,
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
    pub fn new(file: &'a mut crate::fat::Read) -> Loader {
        Loader {
            file,
            num_sections: 0,
            image_base: 0,
            image_size: 0,
        }
    }

    pub fn load(&mut self, address: u64) -> Result<u64, Error> {
        let mut data: [u8; 1024] = [0; 1024];

        match self.file.read(&mut data[0..512]) {
            Ok(_) => {}
            Err(_) => return Err(Error::FileError),
        }

        match self.file.read(&mut data[512..]) {
            Ok(_) => {}
            Err(_) => return Err(Error::FileError),
        }

        let dos_region = MemoryRegion::from_slice(&data);

        // 'MZ' magic
        if dos_region.read_u16(0) != 0x5a4d {
            return Err(Error::InvalidExecutable);
        }

        // offset to COFF header
        let pe_header_offset = dos_region.read_u32(0x3c);

        if pe_header_offset >= 512 {
            return Err(Error::InvalidExecutable);
        }

        let pe_region = MemoryRegion::from_slice(&data[pe_header_offset as usize..]);

        // The Microsoft specification uses offsets relative to the COFF area
        // which is 4 after the signature (so all offsets are +4 relative to the spec)
        // 'PE' magic
        if pe_region.read_u32(0) != 0x0000_4550 {
            return Err(Error::InvalidExecutable);
        }

        // Check for supported machine
        if pe_region.read_u16(4) != 0x8664 {
            return Err(Error::InvalidExecutable);
        }

        self.num_sections = pe_region.read_u16(6);

        let optional_header_size = pe_region.read_u16(20);
        let optional_region = MemoryRegion::from_slice(&data[(24 + pe_header_offset) as usize..]);

        // Only support x86-64 EFI
        if optional_region.read_u16(0) != 0x20b {
            return Err(Error::InvalidExecutable);
        }

        let entry_point = optional_region.read_u32(16);

        self.image_base = optional_region.read_u64(24);
        self.image_size = optional_region.read_u32(56);

        let sections = &data[(24 + pe_header_offset + u32::from(optional_header_size)) as usize..];
        let sections: &[Section] = unsafe {
            core::slice::from_raw_parts(
                sections.as_ptr() as *const Section,
                self.num_sections as usize,
            )
        };

        let mut loaded_region = MemoryRegion::new(address, u64::from(self.image_size));

        for section in sections {
            for x in 0..section.virt_size {
                loaded_region.write_u8(u64::from(x) + u64::from(section.virt_address), 0);
            }

            // TODO: Handle strange offset sections.
            if section.raw_offset % 512 != 0 {
                continue;
            }

            match self.file.seek(section.raw_offset) {
                Ok(_) => {}
                Err(_) => return Err(Error::FileError),
            }

            let mut section_data: [u8; 512] = [0; 512];

            let mut section_offset = 0;
            let section_size = core::cmp::min(section.raw_size, section.virt_size);
            while section_offset < section_size {
                let remaining_bytes = core::cmp::min(section_size - section_offset, 512);
                match self.file.read(&mut section_data) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(Error::FileError);
                    }
                }

                let l: &mut [u8] = loaded_region.as_mut_slice(
                    u64::from(section.virt_address + section_offset),
                    u64::from(remaining_bytes),
                );
                l.copy_from_slice(&section_data[0..remaining_bytes as usize]);
                section_offset += remaining_bytes;
            }
        }
        Ok(address + u64::from(entry_point))
    }
}

#[cfg(test)]
mod tests {
    use crate::part::tests::FakeDisk;

    use std::alloc;

    #[test]
    fn test_loader() {
        let d = FakeDisk::new("clear-28660-kvm.img");
        let (start, end) = crate::part::find_efi_partition(&d).unwrap();

        let mut f = crate::fat::Filesystem::new(&d, start, end);
        f.init().unwrap();
        let mut file = f.open("/EFI/BOOT/BOOTX64 EFI").unwrap();
        let mut l = super::Loader::new(&mut file);

        let fake_mem = unsafe {
            let layout = alloc::Layout::from_size_align(64 * 1024 * 1024, 1024 * 1024).unwrap();
            alloc::alloc(layout)
        };

        let a = l.load(fake_mem as u64).expect("expect loading success");
        assert_eq!(a, fake_mem as u64 + 0x4000);
    }

}
