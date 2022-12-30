// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Akira Moroo

use aarch64_cpu::registers::*;
use tock_registers::interfaces::ReadWriteable;

pub fn setup_simd() {
    CPACR_EL1.modify(CPACR_EL1::FPEN::TrapNothing);
}
