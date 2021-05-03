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

#[macro_export]
macro_rules! offset_of {
    ($container:ty, $field:ident) => {
        #[allow(deref_nullptr)]
        unsafe {
            &(*(core::ptr::null() as *const $container)).$field as *const _ as usize
        }
    };
}

#[macro_export]
macro_rules! container_of {
    ($ptr:ident, $container:ty, $field:ident) => {{
        (($ptr as usize) - offset_of!($container, $field)) as *const $container
    }};
}

#[macro_export]
macro_rules! container_of_mut {
    ($ptr:ident, $container:ty, $field:ident) => {{
        (($ptr as usize) - offset_of!($container, $field)) as *mut $container
    }};
}

// SAFETY: Requires that addr point to a static, null-terminated C-string.
// The returned slice does not include the null-terminator.
pub unsafe fn from_cstring(addr: u64) -> &'static [u8] {
    if addr == 0 {
        return &[];
    }
    let start = addr as *const u8;
    let mut size: usize = 0;
    while start.add(size).read() != 0 {
        size += 1;
    }
    core::slice::from_raw_parts(start, size)
}

pub fn ascii_strip(s: &[u8]) -> &str {
    core::str::from_utf8(s).unwrap().trim_matches(char::from(0))
}

pub fn ucs2_as_ascii_length(input: *const u16) -> usize {
    let mut len = 0;
    loop {
        let v = (unsafe { *(((input as u64) + (2 * len as u64)) as *const u16) } & 0xffu16) as u8;

        if v == 0 {
            break;
        }
        len += 1;
    }
    len
}

pub fn ascii_length(input: &str) -> usize {
    let mut len = 0;
    for c in input.chars() {
        if c == '\0' {
            break;
        }
        len += 1;
    }
    len
}

pub fn ucs2_to_ascii(input: *const u16, output: &mut [u8]) {
    let mut i: usize = 0;
    assert!(output.len() >= ucs2_as_ascii_length(input));
    while i < output.len() {
        unsafe {
            output[i] = (*(((input as u64) + (2 * i as u64)) as *const u16) & 0xffu16) as u8;
        }
        if output[i] == 0 {
            break;
        }
        i += 1;
    }
}

pub fn ascii_to_ucs2(input: &str, output: &mut [u16]) {
    assert!(output.len() >= input.len() * 2);

    for (i, c) in input.bytes().enumerate() {
        output[i] = u16::from(c);
    }
}
