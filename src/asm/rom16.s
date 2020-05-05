.section .rom16, "ax"
.code16

.align 16
rom16_start:
    # Order of instructions from Intel SDM 9.9.1 "Switching to Protected Mode"
    # Step 1: Disable interrupts
    cli

    # Step 2: Load the GDT
    # We are currently in 16-bit real mode. To enter 32-bit protected mode, we
    # need to load 32-bit code/data segments into our GDT. The gdt32 in ROM is
    # at too high of an address (right below 4G) for the data segment to reach.
    #
    # But we can load gdt32 via the code segement. After a reset, the base of
    # the CS register is 0xFFFF0000, which means we can access gdt32.
    lgdtl %cs:(GDT32_PTR - 0xFFFF0000)

    # Step 3: Set CRO.PE (Protected Mode Enable)
    movl %cr0, %eax
    orb  $0b00000001, %al # Set bit 0
    movl %eax, %cr0

    # Step 4: Far JMP to change execution flow and serialize the processor.
    # Set CS to a 32-bit Code-Segment and jump to 32-bit code.
    ljmpl $0x08, $rom32_start
