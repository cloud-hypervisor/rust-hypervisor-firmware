// SPDX-License-Identifier: Apache-2.0
// Copyright 2020 Google LLC

use core::arch::global_asm;

global_asm!(include_str!("ram32.s"), options(att_syntax, raw));
