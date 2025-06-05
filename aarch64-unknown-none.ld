ENTRY(ram64_start)

/* Cloud Hypervisor Memory layout:
    DRAM:   [0x4000_0000-0xfc00_0000]
    FDT:    [0x4000_0000-0x401f_ffff)
    ACPI:   [0x4020_0000-0x403f_ffff)
    payload:[0x4040_0000-0x405f_ffff)
    RHF:    [0x40600000-]
   Assuming 2MB is enough to load payload.
   The stack start is at the end of the RHF region. */
ram_min = 0x40600000;

/* This value must be identical with arch::aarch64::layout::map::dram::KERNEL_START. */
PAYLOAD_START = 0x40400000;

efi_image_size = rhf_end - ram_min;
efi_image_offset = ram_min - PAYLOAD_START;

SECTIONS
{
  /* Mapping the program headers and note into RAM makes the file smaller. */
  . = ram_min;

  /* These sections are mapped into RAM from the file. Omitting :ram from
     later sections avoids emitting empty sections in the final binary.       */
  code_start = .;
  .text.boot  : { *(.text.boot)        }
  .text       : { *(.text .text.*)     }
  . = ALIGN(64K);
  code_end = .;

  data_start = .;
  .data       : { *(.data .data.*)     }
  .rodata     : { *(.rodata .rodata.*) }
  .got        : { *(.got .got.*)       }

  /* The BSS section isn't mapped from file data. It is just zeroed in RAM. */
  .bss : {
    *(.bss .bss.*)
  }
  . = ALIGN(4K);
  data_end = .;

  stack_start = .;
  .stack (NOLOAD) : ALIGN(4K) { . += 128K; }
  stack_end = .;

  /* Strip symbols from the output binary (comment out to get symbols) */
  /DISCARD/ : {
    *(.symtab)
    *(.strtab)
  }

  . = ALIGN(4K);
  rhf_end = .;
}
