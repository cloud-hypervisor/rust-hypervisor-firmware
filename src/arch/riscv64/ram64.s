// Copyright (c) 2021 by Rivos Inc.
// Licensed under the Apache License, Version 2.0, see LICENSE for details.
// SPDX-License-Identifier: Apache-2.0

.option norvc

.section .text.boot

// The entry point for the boot CPU.
.global ram64_start
ram64_start:

.option push
.option norelax
    la gp, __global_pointer$
.option pop
    csrw sstatus, zero
    csrw sie, zero

    la   sp, stack_end
    call rust64_start
wfi_loop:
    wfi
    j    wfi_loop
