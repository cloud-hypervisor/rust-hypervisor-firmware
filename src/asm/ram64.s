.section .text, "ax"
.global linux64_start
.code64

linux64_start:
    # Zero out %rdi, its value is unspecificed in the Linux Boot Protocol.
    xorq %rdi, %rdi

ram64_start:
    # Initialize the stack pointer (Rust code always uses the stack)
    movq $stack_start, %rsp

    # PVH start_info is in %rdi, the first paramter of the System V ABI.
    # BootParams are in %rsi, the second paramter of the System V ABI.
    jmp rust64_start
