ENTRY(_start)

PHDRS
{
  rodata PT_LOAD FILEHDR PHDRS ;
  data   PT_LOAD               ;
  text   PT_LOAD               ;
}

SECTIONS
{
	. = 1M ;
	_start_of_file = . ;

	/* Mapping in the program headers makes it easier to mmap the whole file. */
	. += SIZEOF_HEADERS ;

	.rodata : { *(.rodata .rodata.*) } :rodata
	.data   : { *(.data .data.*)     } :data
	.bss    : { *(.bss .bss.*)       } :data
	.text   : { *(.text .text.*)     } :text

	_end_of_file = . ;

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
