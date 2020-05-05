ENTRY(rust64_start) /* This firmware doesn't use the ELF entrypoint */

/* Loaders like to put stuff in low memory (< 1M), so we don't use it. */
ram_min = 1M;
/* ram32.s only maps the first 2 MiB, which must include our whole program. */
ram_max = 2M;

PHDRS
{
  ram  PT_LOAD FILEHDR PHDRS ;
  note PT_NOTE               ;
  rom  PT_LOAD AT(ram_max)   ;
}

SECTIONS
{
  /* Mapping the program headers and note into RAM makes the file smaller. */
  . = ram_min;
  . += SIZEOF_HEADERS;
  .note : { *(.note) } :note :ram

  /* These sections are mapped into RAM from the file. Omitting :ram from
     later sections avoids emitting empty sections in the final binary.       */
  data_start = .;
  .rodata : { *(.rodata .rodata.*) } :ram
  .text   : { *(.text .text.*)     }
  .text32 : { *(.text32)           }
  .data   : { *(.data .data.*)     }
  data_size = . - data_start;
  file_size = . - ram_min;

  /* The BSS section isn't mapped from file data. It is just zeroed in RAM. */
  .bss : {
    bss_start = .;
    *(.bss .bss.*)
    bss_size = . - bss_start;
  }

  /* Our stack grows down and is page-aligned. TODO: Add stack guard pages. */
  .stack (NOLOAD) : ALIGN(4K) { . += 64K; }
  stack_start = .;
  ASSERT((. <= ram_max), "Stack overflows initial identity-mapped region")

  /* This is correct because all of the code sections have alignment 16. */
  rom_size = ALIGN(SIZEOF(.gdt32), 16) + ALIGN(SIZEOF(.rom32), 16)
           + ALIGN(SIZEOF(.rom16), 16) + ALIGN(SIZEOF(.reset), 16);
  /* Our file's length (RAM code/data + ROM) without any additional padding */
  file_size += rom_size;
  /* QEMU requires the ROM size to be a multiple of 64K. To achieve this for
     our `sstrip`ed binary, we insert enough padding before the ROM code,
     so that the end of the ROM code falls on a 64K file bounary.             */
  pad_size = ALIGN(file_size, 64K) - file_size;

  /* When using the ROM, the entire firmware is loaded right below 4 GiB.
     We work backwards from the end to figure where to place the ROM code,
     32-bit GDT, padding, and the remainder of the firmware.                  */
  pad_start = (1 << 32) - rom_size - pad_size;
  /* The remainder of the firmware code/data expects to be run at addresses
     [data_start, data_start + data_size), but will initially be located in
     ROM, at addresses [rom_data_start, rom_data_start + data_size). As the
     padding comes right after the remainder of the firmware, we have:        */
  rom_data_start = pad_start - data_size;

  /* Only insert the padding if we are building as a ROM (to save size). */
  .pad  pad_start : { . += rom_size ? pad_size : 0; } :NONE
  .gdt32 : { *(.gdt32)       } :rom
  .rom32 : { *(.rom32)       }
  .rom16 : { *(.rom16)       }
  .reset : { KEEP(*(.reset)) }

  /* Strip symbols from the output binary (comment out to get symbols) */
  /DISCARD/ : {
    *(.symtab)
    *(.strtab)
  }
}
