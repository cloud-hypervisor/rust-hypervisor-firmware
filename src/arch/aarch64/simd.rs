// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Akira Moroo

use aarch64_cpu::registers::*;

pub fn setup_simd() {
    CPACR_EL1.modify_no_read(CPACR_EL1.extract(), CPACR_EL1::FPEN::TrapNothing);
}
