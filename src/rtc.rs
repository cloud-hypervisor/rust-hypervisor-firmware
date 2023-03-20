// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

#[cfg(target_arch = "aarch64")]
pub use crate::rtc_pl031::{read_date, read_time};

#[cfg(target_arch = "x86_64")]
pub use crate::cmos::{read_date, read_time};

#[cfg(target_arch = "riscv64")]
pub use crate::rtc_goldfish::{read_date, read_time};
