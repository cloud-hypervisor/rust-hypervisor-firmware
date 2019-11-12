.section .ram64, "ax"
.global ram64_start
.code64

ram64_start:
    # Indicate (via serial) that we are in long/64-bit mode
    movw $0x3f8, %dx
    movb $'L', %al
    outb %al, %dx

    # Enable SSE2 for XMM registers (needed for EFI calling)
    # Clear CR0.EM and Set CR0.MP
    movq %cr0, %rax
    andb $0b11111011, %al # Clear bit 2
    orb  $0b00000010, %al # Set bit 1
    movq %rax, %cr0
    # Set CR4.OSFXSR and CR4.OSXMMEXCPT
    movq %cr4, %rax
    orb  $0b00000110, %ah # Set bits 9 and 10
    movq %rax, %cr4

    # Setup the stack (at the end of our RAM region)
    movq $ram_max, %rsp

    jmp rust64_start

halt_loop:
    hlt
    jmp halt_loop