ENTRY(ram64_start)

PHDRS
{
  rodata PT_LOAD FILEHDR PHDRS ;
  data   PT_LOAD               ;
  text   PT_LOAD               ;
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

	.rodata : { *(.rodata .rodata.*)            } :rodata
	.data   : { *(.data .data.*) *(.bss .bss.*) } :data
	.text   : {
		*(.text .text.*)
		*(.ram64)
	} :text

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
