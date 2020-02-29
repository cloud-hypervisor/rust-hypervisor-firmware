.section .text, "ax"
.global ram64_start
.code64

ram64_start:
    # Indicate (via serial) that we are in long/64-bit mode
    movw $0x3f8, %dx
    movb $'L', %al
    outb %al, %dx

    # Setup the stack (at the end of our RAM region)
    movq $ram_max, %rsp

    jmp rust64_start
