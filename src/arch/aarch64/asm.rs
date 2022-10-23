// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2022 Akira Moroo

use core::arch::global_asm;

global_asm!(include_str!("ram64.s"));
