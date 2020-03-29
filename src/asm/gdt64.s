.section .rodata, "a"

gdt64_ptr:
    .short gdt64_end - gdt64_start - 1 # GDT length is actually (length - 1)
    .quad gdt64_start

gdt64_start: # First descriptor is always null
    .quad 0
code64_desc: # 64-bit Code-Segments always have: Base = 0, Limit = 4G
    # CS.Limit[15:00] = 0                - Ignored
    .short 0x0000
    # CS.Base[15:00]  = 0                - Ignored
    .short 0x0000
    # CS.Base[23:16]  = 0   (bits 0-7)   - Ignored
    .byte 0x00
    # CS.Accessed     = 1   (bit  8)     - Don't write to segment on first use
    # CS.ReadEnable   = 1   (bit  9)     - Read/Execute Code-Segment
    # CS.Conforming   = 0   (bit  10)    - Nonconforming, no lower-priv access
    # CS.Executable   = 1   (bit  11)    - Code-Segement
    # CS.S            = 1   (bit  12)    - Not a System-Segement
    # CS.DPL          = 0   (bits 13-14) - We only use this segment in Ring 0
    # CS.P            = 1   (bit  15)    - Segment is present
    .byte 0b10011011
    # CS.Limit[19:16] = 0   (bits 16-19) - Ignored
    # CS.AVL          = 0   (bit  20)    - Our software doesn't use this bit
    # CS.L            = 1   (bit  21)    - This isn't a 64-bit segment
    # CS.D            = 0   (bit  22)    - This is a 32-bit segment
    # CS.G            = 0   (bit  23)    - Ignored
    .byte 0b00100000
    # CS.Base[31:24]  = 0   (bits 24-31) - Ignored
    .byte 0x00
gdt64_end:
