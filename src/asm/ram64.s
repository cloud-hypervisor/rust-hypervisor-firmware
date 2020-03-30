.section .text, "ax"
.global ram64_start
.code64

ram64_start:
    # Setup the stack (at the end of our RAM region)
    movq $ram_max, %rsp

    # BootParams are in %rsi, the second paramter of the System V ABI.
    jmp rust64_start
