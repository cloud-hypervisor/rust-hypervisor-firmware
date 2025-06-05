ENTRY(ram64_start)

/* OpenSBI loads here */
ram_min = 0x80200000;

SECTIONS
{
  /* Mapping the program headers and note into RAM makes the file smaller. */
  . = ram_min;

  /* These sections are mapped into RAM from the file. Omitting :ram from
     later sections avoids emitting empty sections in the final binary.       */
  code_start = .;
  .text.boot  : { *(.text.boot)        }
  .text       : { *(.text .text.*)     }
  . = ALIGN(4K);
  code_end = .;

  data_start = .;

  .data : {
    . = ALIGN(4096);
    *(.data .data.*)
    . = ALIGN(8);
    PROVIDE(__global_pointer$ = . + 0x800);
  }

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
}
