.section .reset, "ax"
.code16

# The reset vector must go at the end of ROM, exactly 16 bytes from the end.
.align 16
reset_vec: # 0x0_FFFF_FFF0
    jmp rom16_start
 
.align 16, 0
reset_end: # 0x1_0000_0000