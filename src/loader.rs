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

use crate::{
    boot,
    bzimage::{self, Kernel},
    common::ascii_strip,
    fat::{self, Read},
};

pub struct LoaderConfig {
    pub bzimage_path: [u8; 260],
    pub initrd_path: [u8; 260],
    pub cmdline: [u8; 4096],
}

#[derive(Debug)]
pub enum Error {
    FileError(fat::Error),
    BzImageError(bzimage::Error),
}

impl From<fat::Error> for Error {
    fn from(e: fat::Error) -> Error {
        Error::FileError(e)
    }
}

impl From<bzimage::Error> for Error {
    fn from(e: bzimage::Error) -> Error {
        Error::BzImageError(e)
    }
}

fn default_entry_file(f: &mut fat::File) -> Result<[u8; 260], fat::Error> {
    let mut data = [0; 4096];
    assert!(f.get_size() as usize <= data.len());

    let mut entry_file_name = [0; 260];
    let mut offset = 0;
    loop {
        match f.read(&mut data[offset..offset + 512]) {
            Err(fat::Error::EndOfFile) => break,
            Err(e) => return Err(e),
            Ok(_) => {
                offset += 512;
            }
        }
    }

    let conf = unsafe { core::str::from_utf8_unchecked(&data) };
    for line in conf.lines() {
        if let Some(entry) = line.strip_prefix("default") {
            let entry = entry.trim();
            entry_file_name[0..entry.len()].copy_from_slice(entry.as_bytes());
        }
    }

    Ok(entry_file_name)
}

fn parse_entry(f: &mut fat::File) -> Result<LoaderConfig, fat::Error> {
    let mut data = [0; 4096];
    assert!(f.get_size() as usize <= data.len());

    let mut loader_config: LoaderConfig = unsafe { core::mem::zeroed() };

    let mut offset = 0;
    loop {
        match f.read(&mut data[offset..offset + 512]) {
            Err(fat::Error::EndOfFile) => break,
            Err(e) => return Err(e),
            Ok(_) => {
                offset += 512;
            }
        }
    }

    let conf = unsafe { core::str::from_utf8_unchecked(&data) };
    for line in conf.lines() {
        if let Some(entry) = line.strip_prefix("linux") {
            let entry = entry.trim();
            loader_config.bzimage_path[0..entry.len()].copy_from_slice(entry.as_bytes());
        }
        if let Some(entry) = line.strip_prefix("options") {
            let entry = entry.trim();
            loader_config.cmdline[0..entry.len()].copy_from_slice(entry.as_bytes());
        }
        if let Some(entry) = line.strip_prefix("initrd") {
            let entry = entry.trim();
            loader_config.initrd_path[0..entry.len()].copy_from_slice(entry.as_bytes());
        }
    }

    Ok(loader_config)
}

const ENTRY_DIRECTORY: &str = "/loader/entries/";

fn default_entry_path(fs: &fat::Filesystem) -> Result<[u8; 260], fat::Error> {
    let mut f = match fs.open("/loader/loader.conf")? {
        fat::Node::File(f) => f,
        _ => return Err(fat::Error::NotFound),
    };
    let default_entry = default_entry_file(&mut f)?;
    let default_entry = ascii_strip(&default_entry);

    let mut entry_path = [0u8; 260];
    entry_path[0..ENTRY_DIRECTORY.len()].copy_from_slice(ENTRY_DIRECTORY.as_bytes());

    entry_path[ENTRY_DIRECTORY.len()..ENTRY_DIRECTORY.len() + default_entry.len()]
        .copy_from_slice(default_entry.as_bytes());
    Ok(entry_path)
}

pub fn load_default_entry(fs: &fat::Filesystem, info: &dyn boot::Info) -> Result<Kernel, Error> {
    let default_entry_path = default_entry_path(&fs)?;
    let default_entry_path = ascii_strip(&default_entry_path);

    let mut f = match fs.open(default_entry_path)? {
        fat::Node::File(f) => f,
        _ => return Err(Error::FileError(fat::Error::NotFound)),
    };
    let entry = parse_entry(&mut f)?;

    let bzimage_path = ascii_strip(&entry.bzimage_path);
    let initrd_path = ascii_strip(&entry.initrd_path);
    let cmdline = ascii_strip(&entry.cmdline);

    let mut kernel = Kernel::new(info);

    let mut bzimage_file = fs.open(bzimage_path)?;
    kernel.load_kernel(&mut bzimage_file)?;

    if !initrd_path.is_empty() {
        let mut initrd_file = fs.open(initrd_path)?;
        kernel.load_initrd(&mut initrd_file)?;
    }

    kernel.append_cmdline(info.cmdline());
    kernel.append_cmdline(cmdline.as_bytes());

    Ok(kernel)
}

#[cfg(test)]
mod tests {
    use crate::fat::Read;
    use crate::part::tests::FakeDisk;
    use core::convert::TryInto;

    #[test]
    fn test_default_entry() {
        let d = FakeDisk::new("clear-28660-kvm.img");
        let (start, end) = crate::part::find_efi_partition(&d).unwrap();
        let mut fs = crate::fat::Filesystem::new(&d, start, end);
        fs.init().expect("Error initialising filesystem");

        let mut f: crate::fat::File = fs.open("/loader/loader.conf").unwrap().try_into().unwrap();
        let s = super::default_entry_file(&mut f).unwrap();
        let s = super::ascii_strip(&s);
        assert_eq!(s, "Clear-linux-kvm-5.0.6-318");

        let default_entry_path = super::default_entry_path(&fs).unwrap();
        let default_entry_path = super::ascii_strip(&default_entry_path);

        assert_eq!(
            default_entry_path,
            format!("/loader/entries/{}", s).as_str()
        );

        let mut f: crate::fat::File = fs.open(default_entry_path).unwrap().try_into().unwrap();
        let entry = super::parse_entry(&mut f).unwrap();
        let s = super::ascii_strip(&entry.bzimage_path);
        assert_eq!(s, "/EFI/org.clearlinux/kernel-org.clearlinux.kvm.5.0.6-318");
        let s = super::ascii_strip(&entry.cmdline);
        let s = s.trim_matches(char::from(0));
        assert_eq!(s, "root=PARTUUID=ae06d187-e9fc-4d3b-9e5b-8e6ff28e894f console=tty0 console=ttyS0,115200n8 console=hvc0 quiet init=/usr/lib/systemd/systemd-bootchart initcall_debug tsc=reliable no_timer_check noreplace-smp cryptomgr.notests rootfstype=ext4,btrfs,xfs kvm-intel.nested=1 rw");
    }
}
