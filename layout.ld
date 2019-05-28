ENTRY(_start)
 
SECTIONS
{
	. = 1M;
	start_of_text = . ;
	.text : ALIGN(4K)
	{
		*(.text .text.*)
	}
	end_of_text = . ;

	start_of_data = . ;
	.rodata : ALIGN(4K)
	{
		*(.rodata .rodata.*)
	}
 
	.data : ALIGN(4K)
	{
		*(.data .data.*)
	}
	end_of_data = . ;
 
	.bss : ALIGN(4K)
	{
		*(COMMON)
		*(.bss .bss.*)
	}
}
