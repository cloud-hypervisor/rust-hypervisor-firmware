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
    block::SectorBuf,
    boot,
    bzimage::{self, Kernel},
    common::ascii_strip,
    fat::{self, Read},
};

const ENTRY_DIRECTORY: &str = "/loader/entries";

pub struct LoaderConfig {
    pub bzimage_path: [u8; 260],
    pub initrd_path: [u8; 260],
    pub cmdline: [u8; 4096],
}

#[derive(Debug)]
pub enum Error {
    File(fat::Error),
    BzImage(bzimage::Error),
    UnterminatedString,
}

impl From<fat::Error> for Error {
    fn from(e: fat::Error) -> Error {
        Error::File(e)
    }
}

impl From<bzimage::Error> for Error {
    fn from(e: bzimage::Error) -> Error {
        Error::BzImage(e)
    }
}

/// Given a `loader.conf` file, find the `default` option value.
fn default_entry_pattern(f: &mut fat::File) -> Result<[u8; 260], fat::Error> {
    let mut data = [0; 4096];
    assert!(f.get_size() as usize <= data.len());
    assert!(data.len() >= SectorBuf::len());

    let mut entry_pattern = [0; 260];
    let mut offset = 0;
    loop {
        match f.read(&mut data[offset..offset + SectorBuf::len()]) {
            Err(fat::Error::EndOfFile) => break,
            Err(e) => return Err(e),
            Ok(_) => {
                offset += SectorBuf::len();
            }
        }
    }

    let conf = unsafe { core::str::from_utf8_unchecked(&data) };
    for line in conf.lines() {
        if let Some(mut pattern) = line.strip_prefix("default") {
            pattern = pattern.trim();
            entry_pattern[0..pattern.len()].copy_from_slice(pattern.as_bytes());
        }
    }

    Ok(entry_pattern)
}

/// Given a glob-like pattern, select a boot entry from `/loader/entries/`,
/// falling back to the first entry encountered if no match is found.
fn find_entry(fs: &fat::Filesystem, pattern: &[u8]) -> Result<[u8; 255], Error> {
    let mut dir: fat::Directory = fs.open(ENTRY_DIRECTORY)?.try_into()?;
    let mut fallback = None;
    loop {
        match dir.next_entry() {
            Ok(de) => {
                if !de.is_file() {
                    continue;
                }
                let file_name = de.long_name();
                // return the first matching file name
                if compare_entry(&file_name, pattern)? {
                    return Ok(file_name);
                }
                // only fallback to entry files ending with `.conf`
                if fallback.is_none() && compare_entry(&file_name, b"*.conf\0")? {
                    fallback = Some(file_name);
                }
            }
            Err(fat::Error::EndOfFile) => break,
            Err(err) => return Err(err.into()),
        }
    }
    fallback.ok_or_else(|| fat::Error::NotFound.into())
}

/// Attempt to match a file name with a glob-like pattern.
/// An error is returned if either `file_name` or `pattern` are not `\0`
/// terminated.
fn compare_entry(file_name: &[u8], pattern: &[u8]) -> Result<bool, Error> {
    fn compare_entry_inner<I>(
        mut name_iter: core::iter::Peekable<I>,
        mut pattern: &[u8],
        max_depth: usize,
    ) -> Result<bool, Error>
    where
        I: Iterator<Item = u8> + Clone,
    {
        if max_depth == 0 {
            return Ok(false);
        }
        while let Some(p) = pattern.take_first() {
            let f = name_iter.peek().ok_or(Error::UnterminatedString)?;
            #[cfg(test)]
            println!("{} ~ {}", *p as char, *f as char);
            match p {
                b'\0' => return Ok(*f == b'\0'),
                b'\\' => {
                    match pattern.take_first() {
                        // trailing escape
                        Some(b'\0') | None => return Ok(false),
                        // no match
                        Some(p) if p != f => return Ok(false),
                        // continue
                        _ => (),
                    }
                }
                b'?' => {
                    if *f == b'\0' {
                        return Ok(false);
                    }
                }
                b'*' => {
                    while name_iter.peek().is_some() {
                        if compare_entry_inner(name_iter.clone(), pattern, max_depth - 1)? {
                            return Ok(true);
                        }
                        name_iter.next().ok_or(Error::UnterminatedString)?;
                    }
                    return Ok(*pattern.first().ok_or(Error::UnterminatedString)? == b'\0');
                }
                // TODO
                b'[' => todo!("patterns containing `[...]` sets are not supported"),
                _ if p != f => return Ok(false),
                _ => (),
            }
            name_iter.next().ok_or(Error::UnterminatedString)?;
        }
        Ok(false)
    }
    let name_iter = file_name.iter().copied().peekable();
    compare_entry_inner(name_iter, pattern, 32)
}

fn parse_entry(f: &mut fat::File) -> Result<LoaderConfig, fat::Error> {
    let mut data = [0; 4096];
    assert!(f.get_size() as usize <= data.len());
    assert!(data.len() >= SectorBuf::len());

    let mut loader_config: LoaderConfig = unsafe { core::mem::zeroed() };

    let mut offset = 0;
    loop {
        match f.read(&mut data[offset..offset + SectorBuf::len()]) {
            Err(fat::Error::EndOfFile) => break,
            Err(e) => return Err(e),
            Ok(_) => {
                offset += SectorBuf::len();
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

fn default_entry_path(fs: &fat::Filesystem) -> Result<[u8; 260], Error> {
    let mut f = match fs.open("/loader/loader.conf")? {
        fat::Node::File(f) => f,
        _ => return Err(fat::Error::NotFound.into()),
    };
    let default_entry_pattern = default_entry_pattern(&mut f)?;

    let default_entry = find_entry(fs, &default_entry_pattern)?;
    let default_entry = ascii_strip(&default_entry);

    let mut entry_path = [0u8; 260];
    entry_path[0..ENTRY_DIRECTORY.len()].copy_from_slice(ENTRY_DIRECTORY.as_bytes());
    entry_path[ENTRY_DIRECTORY.len()] = b'/';
    entry_path[ENTRY_DIRECTORY.len() + 1..ENTRY_DIRECTORY.len() + default_entry.len() + 1]
        .copy_from_slice(default_entry.as_bytes());
    Ok(entry_path)
}

pub fn load_default_entry(fs: &fat::Filesystem, info: &dyn boot::Info) -> Result<Kernel, Error> {
    let default_entry_path = default_entry_path(fs)?;
    let default_entry_path = ascii_strip(&default_entry_path);

    let mut f = match fs.open(default_entry_path)? {
        fat::Node::File(f) => f,
        _ => return Err(Error::File(fat::Error::NotFound)),
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
    use crate::part::tests::*;
    use core::convert::TryInto;

    #[test]
    fn test_default_entry() {
        let d = FakeDisk::new(&clear_disk_path());
        let (start, end) = crate::part::find_efi_partition(&d).unwrap();
        let mut fs = crate::fat::Filesystem::new(&d, start, end);
        fs.init().expect("Error initialising filesystem");

        let mut f: crate::fat::File = fs.open("/loader/loader.conf").unwrap().try_into().unwrap();
        let s = super::default_entry_pattern(&mut f).unwrap();
        let s = super::ascii_strip(&s);
        assert_eq!(s, "Clear-linux-kvm-5.0.6-318");

        let default_entry_path = super::default_entry_path(&fs).unwrap();
        let default_entry_path = super::ascii_strip(&default_entry_path);

        assert_eq!(
            default_entry_path,
            format!("/loader/entries/{}.conf", s).as_str()
        );

        let mut f: crate::fat::File = fs.open(default_entry_path).unwrap().try_into().unwrap();
        let entry = super::parse_entry(&mut f).unwrap();
        let s = super::ascii_strip(&entry.bzimage_path);
        assert_eq!(s, "/EFI/org.clearlinux/kernel-org.clearlinux.kvm.5.0.6-318");
        let s = super::ascii_strip(&entry.cmdline);
        let s = s.trim_matches(char::from(0));
        assert_eq!(s, "root=PARTUUID=ae06d187-e9fc-4d3b-9e5b-8e6ff28e894f console=tty0 console=ttyS0,115200n8 console=hvc0 quiet init=/usr/lib/systemd/systemd-bootchart initcall_debug tsc=reliable no_timer_check noreplace-smp cryptomgr.notests rootfstype=ext4,btrfs,xfs kvm-intel.nested=1 rw");
    }

    macro_rules! entry_pattern_matches {
        (match $entry:literal with {
            $(
                $( #[$attr:meta] )*
                $id:ident: $pat:literal => $result:literal
            ),* $(,)?
        } ) => {
            mod entry_pattern {
                $(
                    #[test]
                    $( #[$attr] )*
                    fn $id() {
                        assert_eq!(super::super::compare_entry($entry, $pat).unwrap(), $result);
                    }
                )*
            }
        }
    }

    entry_pattern_matches! {
        match b"foobar.conf\0" with {
            empty: b"\0" => false,

            exact: b"foobar.conf\0" => true,
            inexact: b"barfoo.conf\0" => false,

            wildcard: b"*\0" => true,
            leading_wildcard: b"*.conf\0" => true,
            internal_wildcard: b"foo*.conf\0" => true,
            trailing_wildcard: b"foob*\0" => true,
            mismatched_wildcard: b"bar*\0" => false,
            wildcard_backtrack: b"*obar.conf\0" => true,

            single_wildcard: b"fo?bar.conf\0" => true,
            mismatched_single_wildcard: b"foo?bar.conf\0" => false,

            escaped_regular_char: b"foo\\bar.conf\0" => true,
            escaped_special_char: b"foo\\?ar.conf\0" => false,
            trailing_escape: b"foobar.conf\\\0" => false,
        }
    }
}
