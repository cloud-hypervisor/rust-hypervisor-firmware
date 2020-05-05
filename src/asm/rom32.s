.section .rom32, "ax"
.code32

.align 16
rom32_start:
    # Now that we are in 32-bit mode, setup all the Data-Segments to be 32-bit.
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs

    # Needed for the REP instructions below
    cld

copy_rom_to_ram:
    # This is equivalent to: memcpy(data_start, rom_data_start, data_size)
    movl $rom_data_start, %esi
    movl $data_start, %edi
    movl $data_size, %ecx
    rep movsb (%esi), (%edi)

zero_bss_in_ram:
    # This is equivalent to: memset(bss_start, 0, bss_size)
    xorb %al, %al
    movl $bss_start, %edi
    movl $bss_size, %ecx
    rep stosb %al, (%edi)

jump_to_ram:
    # Zero out %ebx, as we don't have a PVH StartInfo struct.
    xorl %ebx, %ebx

    # Jumping all that way from ROM (~4 GiB) to RAM (~1 MiB) is too far for a
    # 32-bit relative jump, so we use a 32-bit absolute jump.
    movl $ram32_start, %eax
    jmp *%eax
