ENTRY(ram64_start)

/* Cloud Hypervisor Memory layout:
    DRAM:   [0x4000_0000-0xfc00_0000]
    FDT:    [0x4000_0000-0x401f_ffff)
    ACPI:   [0x4020_0000-0x403f_ffff)
    kernel: [0x4048_0000-]
   The stack start is at the end of the DRAM region. */
ram_min = 0x40480000;

SECTIONS
{
  /* Mapping the program headers and note into RAM makes the file smaller. */
  . = ram_min;

  /* These sections are mapped into RAM from the file. Omitting :ram from
     later sections avoids emitting empty sections in the final binary.       */
  text_start = .;
  .text.boot  : { *(.text.boot)        }
  .text       : { *(.text .text.*)     }
  . = ALIGN(4K);
  text_end = .;
  .data       : { *(.data .data.*)     }
  .rodata     : { *(.rodata .rodata.*) }

  /* The BSS section isn't mapped from file data. It is just zeroed in RAM. */
  .bss : {
    bss_start = .;
    *(.bss .bss.*)
    bss_size = . - bss_start;
  }

  stack_start = .;
  .stack (NOLOAD) : ALIGN(4K) { . += 128K; }
  stack_end = .;

  /* Strip symbols from the output binary (comment out to get symbols) */
  /DISCARD/ : {
    *(.symtab)
    *(.strtab)
  }
}
