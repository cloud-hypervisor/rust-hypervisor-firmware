ENTRY(_start)

SECTIONS
{
	. = 1M ;
	start_of_data = . ;
	.rodata : { *(.rodata .rodata.*) }
	.data : { *(.data .data.*) }
	.bss : { *(COMMON) *(.bss .bss.*) }
	end_of_data = . ;

	start_of_text = . ;
	.text : { *(.text .text.*) }
	end_of_text = . ;
}
