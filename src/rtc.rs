// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

#[cfg(target_arch = "x86_64")]
pub use crate::cmos::{read_date, read_time};

#[cfg(target_arch = "aarch64")]
pub fn read_date() -> Result<(u8, u8, u8), ()> {
    todo!();
}

#[cfg(target_arch = "aarch64")]
pub fn read_time() -> Result<(u8, u8, u8), ()> {
    todo!();
}
