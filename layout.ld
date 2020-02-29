ENTRY(ram64_start)

PHDRS
{
  program PT_LOAD FILEHDR PHDRS ;
}

/* Loaders like to put stuff in low memory (< 1M), so we don't use it. */
ram_min = 1M;
ram_max = 2M;
/* Our stack grows down from ram_max. TODO: Add a guard for stack overflows. */
stack_size = 64K;

SECTIONS
{
	/* Mapping in the program headers makes it easier to mmap the whole file. */
	. = ram_min;
	. += SIZEOF_HEADERS;

	.rodata : { *(.rodata .rodata.*) } :program
	.text   : { *(.text .text.*)     } :program
	.data   : { *(.data .data.*)     } :program
	.bss    : { *(.bss .bss.*)       } :program

	firmware_ram_size = . - ram_min;

	ASSERT((. <= ram_max - stack_size), "firmware size too big for RAM region")

	/* Match edk2's GccBase.lds DISCARD section */
	/DISCARD/ : {
		*(.note.GNU-stack)
		*(.gnu_debuglink)
		*(.interp)
		*(.dynsym)
		*(.dynstr)
		*(.dynamic)
		*(.hash .gnu.hash)
		*(.comment)
		*(COMMON)
	}
}
