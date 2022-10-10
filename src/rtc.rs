// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

#[cfg(target_arch = "x86_64")]
pub use crate::cmos::{read_date, read_time};
