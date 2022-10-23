/* SPDX-License-Identifier: Apache-2.0 */
/* Copyright (C) 2022 Akira Moroo */

.section .text.boot, "ax"
.global ram64_start

ram64_start:
  /*
   * This header follows the AArch64 Linux kernel image header [1] to load
   * as a PE binary by the hypervisor.
   *
   * [1] https://docs.kernel.org/arm64/booting.html#call-the-kernel-image
   */
  add x13, x18, #0x16   /* code0: UEFI "MZ" signature magic instruction */
  b jump_to_rust        /* code1 */

  .quad 0               /* text_offset */
  .quad 0               /* image_size */
  .quad 0               /* flags */
  .quad 0               /* res2 */
  .quad 0               /* res3 */
  .quad 0               /* res4 */

  .long 0x644d5241      /* "ARM\x64" magic number */
  .long 0               /* res5 */
  .align 3

jump_to_rust:
  /* x0 typically points to device tree at entry */
  ldr x0, =0x40000000

  /* setup stack */
  ldr x30, =stack_end
  mov sp, x30

  /* x0: pointer to device tree */
  b rust64_start