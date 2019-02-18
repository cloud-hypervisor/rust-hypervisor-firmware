ENTRY(_start)
 
SECTIONS
{
	. = 1M;
	start_of_text = . ;
	.text BLOCK(4K) : ALIGN(4K)
	{
		*(.text)
	}
	end_of_text = . ;

	start_of_data = . ;
	.rodata BLOCK(4K) : ALIGN(4K)
	{
		*(.rodata)
	}
 
	.data BLOCK(4K) : ALIGN(4K)
	{
		*(.data)
	}
	end_of_data = . ;
 
	.bss BLOCK(4K) : ALIGN(4K)
	{
		*(COMMON)
		*(.bss)
	}
}
