.section .text32, "ax"
.code32

ram32_start:
    # Stash the PVH start_info struct in %rdi.
    movl %ebx, %edi
    # Zero out %rsi, its value is unspecificed in the PVH Boot Protocol.
    xorl %esi, %esi

setup_page_tables:
    # First L2 entry identity maps [0, 2 MiB)
    movl $0b10000011, (L2_TABLES) # huge (bit 7), writable (bit 1), present (bit 0)
    # First L3 entry points to L2 table
    movl $L2_TABLES, %eax
    orb  $0b00000011, %al # writable (bit 1), present (bit 0)
    movl %eax, (L3_TABLE)
    # First L4 entry points to L3 table
    movl $L3_TABLE, %eax
    orb  $0b00000011, %al # writable (bit 1), present (bit 0)
    movl %eax, (L4_TABLE)

enable_paging:
    # Load page table root into CR3
    movl $L4_TABLE, %eax
    movl %eax, %cr3

    # Set CR4.PAE (Physical Address Extension)
    movl %cr4, %eax
    orb  $0b00100000, %al # Set bit 5
    movl %eax, %cr4
    # Set EFER.LME (Long Mode Enable)
    movl $0xC0000080, %ecx
    rdmsr
    orb  $0b00000001, %ah # Set bit 8
    wrmsr
    # Set CRO.PG (Paging)
    movl %cr0, %eax
    orl  $(1 << 31), %eax
    movl %eax, %cr0

jump_to_64bit:
    # We are now in 32-bit compatibility mode. To enter 64-bit mode, we need to
    # load a 64-bit code segment into our GDT.
    lgdtl gdt64_ptr
    # Set CS to a 64-bit segment and jump to 64-bit code.
    ljmpl $(code64_desc - gdt64_start), $ram64_start
