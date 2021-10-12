ENTRY(_start)

SECTIONS {
	. = 0xF000;
	.rodata : ALIGN(4)
	{
		*(.rodata.multiboot_header);
		*(.rodata*);
	}
	.bss : ALIGN(0x1000)
	{
		*(.bss*);
	}

	.data :
	{
		*(.data*);
	}

	.text :
	{
		*(.text*);
	}
}
