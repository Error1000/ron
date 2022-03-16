.intel_syntax noprefix
.global l1_pt
.global l2_pt
.global l3_pt
.global _start
.global end_init

kernel_stack_size = 8*1024
.section .bss
	# NOTE for future helpless programmers: this needs to be aligned so that movups and movaps
	# don't general protection fault when used when the stack pointer is used
	# because the compiler assumes the stack pointer is aligned
	#
	# https://stackoverflow.com/questions/67243284/why-movaps-causes-segmentation-fault
	# From Intel® 64 and IA-32 architectures software developer’s manual, MOVAPS specification:
    # 	MOVAPS—Move *Aligned* Packed Single-Precision Floating-Point Values
    # 	When the source or destination operand is a memory operand, the operand must be aligned on a 16-byte (128-bit version), 32-byte (VEX.256 encoded version) or 64-byte (EVEX.512 encoded version) boundary or a general protection exception (#GP) will be generated.
	.lcomm kernel_stack, kernel_stack_size

	# tables must be page aligned
	# Modes that we are interested in: Protected, Compatibility, 64-bit(long mode), 
	# This is with PAE, compatibility/protected mode 3-level paging ( not 4-level, that is PAE with long mode )
	# Without PAE we can't have long mode and in compatiblity/protected mode it is only 2-level paging
	# 4 gb per level 3 table
      
    # According to the AMD64 programmer's manual vol. 2, section 5.2: "The page-directory-pointer table is aligned on a 32-byte boundary"
	.balign 4*8
	l3_pt:
	.fill 4, 8, 0
	
	# 1 gb per level 2 table
	.balign 512*8
	l2_pt:
	.fill 512, 8, 0

	# 2 mb per level 1 table
	.balign 512*8
	l1_pt: 
	.fill 512, 8, 0
	

.section .data
	.align 4, 0
	gdtr:
		.word 4*8-1
		.long gdt

	.align 4, 0
	gdt:
		# Entry 0
		.hword 0
		.hword 0
		.hword 0 
		.hword 0 

		# Another empty entry because of UEFI
		.hword 0
		.hword 0
		.hword 0 
		.hword 0 

		# Code Segment (cs) entry
		.hword 0xffff # limit
		
		.hword 0x0000 # base
		.byte 0b00000000 # base

		.byte 0b10011010 # access byte
		.byte 0b11111100; # flags ( 4bits ) and limit cont. ( 4bits )
		.byte 0b00000000 # base cont.

		# Data Segment (ds) entry
		.hword 0xffff # limit
		
		.hword 0x0000 # base
		.byte 0b00000000 # base

		.byte 0b10010010 # access byte
		.byte 0b11111100 # flags ( 4bits ) and limit cont. ( 4bits )
		.byte 0b00000000 # base cont.

.section .text
	.extern main
_start:
	cli # disable interrupts
	cld # clear direction

load_gdt:
	# Set up gdt
	lgdt [gdtr]

	# Load segment registers
	mov ecx, 8*3
	mov ss, ecx
	mov ds, ecx
	mov es, ecx
	mov gs, ecx
	mov fs, ecx
	ljmp 8*2, finish_gdt # setting cs
finish_gdt: # Only the finest gdt finland can offer :P
	
	# Set up stack
	mov esp, OFFSET kernel_stack+kernel_stack_size

	push ebx
	push eax

setup_paging:
	# According to the amd64 manual, section 5.2.3, enteries are 8 bytes long, with PAE
	# Level 1 is the lowest

	# One 1 gb page directory
	DEFAULT_L3_ENTRY = 1 # Just the present bit(bit 0), *NOT* the cache disable bit(bit 4) and *NOT* the write through bit(bit 3) 
	mov eax, OFFSET l2_pt
	or eax, DEFAULT_L3_ENTRY # According to the AMD64 manual section 5.4, only the address bits *above* bit 11 are stored in the address field of the entery specifed at section 5.2 page 139
	mov dword ptr [l3_pt], eax

	# Here, since for now we emulate paging in the actual os we can just give access to the pages to everybody as far as the cpu is concerned

	# One, 2mb page table
	DEFAULT_L2_ENTRY = (1 | (1 << 1) | (1 << 2) ) # The present bit(bit 0), the r/w bit(bit 1), the user access bit(bit 2), *NOT* the write through bit(bit 3), *NOT* the page cache disable bit(bit 4), bit 5 is set by the cpu it indicates access, bit 6 must be ignored, bit 7 must be 0, bit 8 must be ignored, bits 11-9 are available for os use
	mov eax, DEFAULT_L2_ENTRY
	or eax, OFFSET l1_pt
	mov dword ptr [l2_pt], eax

	# All 512, 4kb pages of the first and only 2mb page table
	DEFAULT_L1_ENTRY = (1 | (1 << 1) | (1 << 2) ) # The present bit(bit 0), the r/w bit(bit 1), the user ccess bit(bit 2), *NOT* the write thorugh bit(bit 3), *NOT* the cache disable bit(bit 4), bit 5 is set by the cpu it indicates access, bit 6 is set by the cpu it indicates dirtiness, *NOT* Page Attribute Table (bit 7), *NOT* the global bit(bit 8) we want the tlb(cache) to be flushed of this entery  when cr3 changes otherwise we would need to call invlpg to flush the tlb explicitly, bits 9-11 are available for os use
	mov ebx, 0
	l1_loop:
		mov eax, DEFAULT_L1_ENTRY
		shl ebx, 12
		or  eax, ebx # eax = ( DEFAULT_L1_ENTRY | (ebx << 12) ), not shifted back because ebx was not originally an address
		shr ebx, 9
		# size of one entry = 8 => offset = ebx*8 or ebx << 3 <=> (ebx >> 12) << 9
		mov dword ptr [l1_pt+ebx], eax
		shr ebx, 3 # restore ebx
		inc ebx
		cmp ebx, 512 # 512 enteries
	jl l1_loop
	
enable_pae:
        # Enable Physical Address Extension
        mov eax, cr4
        or ax, 1 << 5
        mov cr4, eax
enable_paging:
	# Put a pointer to the Page-Directory-Pointer(a.k.a l3_pt) into cr3
	mov eax, OFFSET l3_pt
	# PCD(Page-Level Cache Disable)/ bit 4 = 0, we want cache :)
	# PWT(Page-Level Write Through)/ bit 3 = 0, a.k.a writeback policy which means on unexpected system shutdown, if the caches do not get flushed info might be lost
	mov cr3, eax

	# Enable paging
	mov eax, cr0
    or eax, (1 << 31)
	mov cr0, eax

enable_simd:
	mov eax, cr0
	and ax, 0xFFFB	# ~(1<<2) # disable coprocessor(old floating point) emulation CR0.EM
	or ax, 0x2			    # enable coprocessor monitoring  CR0.MP
	mov cr0, eax

	mov eax, cr4
	or ax, 0b11 << 9		# enable CR4.OSFXSR and CR4.OSXMMEXCPT, enables simd and unmasked simd floating point exceptions
	mov cr4, eax

goto_kmain:
	call main
