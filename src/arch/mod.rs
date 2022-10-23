// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;
