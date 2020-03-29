ENTRY(linux64_start)

PHDRS
{
  ram  PT_LOAD FILEHDR PHDRS ;
}

/* Loaders like to put stuff in low memory (< 1M), so we don't use it. */
ram_min = 1M;
ram_max = 2M;
/* Our stack grows down from ram_max. TODO: Add a guard for stack overflows. */
stack_size = 64K;

SECTIONS
{
	/* Mapping the program headers into RAM makes the file smaller. */
	. = ram_min;
	. += SIZEOF_HEADERS;

	/* These sections are mapped into RAM from the file. Omitting :ram from
	   later sections avoids emitting empty sections in the final binary.     */
	data_start = .;
	.rodata : { *(.rodata .rodata.*) } :ram
	.text   : { *(.text .text.*)     }
	.text32 : { *(.text32)           }
	.data   : { *(.data .data.*)     }
	data_size = . - data_start;

	/* The BSS section isn't mapped from any file data. It is simply zeroed
	   in RAM. So our file size should be computed from here.                 */
	file_size = . - ram_min;
	.bss : {
		bss_start = .;
		*(.bss .bss.*)
		bss_size = . - bss_start;
	}

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
