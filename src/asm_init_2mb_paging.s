.intel_syntax noprefix
.global _start

kernel_stack_size = 16384
.section .bss
	# tables must be page aligned
	# Modes that we are interested in: Protected/Compatibility, 64-bit(long mode), 
	# This is with PAE, compatibility/protected mode 3-level paging ( not 4-level, that is PAE with long mode )
	# Without PAE we can't have long mode and in compatiblity/protected mode it is only 2-level paging
	# 4 gb per level 3 table
      
        # According to the AMD64 programmer's manual vol. 2, section 5.2: "The page-directory-pointer table is aligned on a 32-byte boundary"
	.balign 4*8
	l3_pt:
	.fill 4, 8, 0
	
	# 1 gb per level 2 table
	.balign 512*8
	l2_pt_1:
	.fill 512, 8, 0

	.balign 512*8
	l2_pt_2:
	.fill 512, 8, 0

	.balign 512*8
	l2_pt_3:
	.fill 512, 8, 0

	.balign 512*8
	l2_pt_4:
	.fill 512, 8, 0

	.lcomm kernel_stack, kernel_stack_size

.section .data
	.align 4, 0
	gdtr:
		.word 3*8-1
		.long gdt

	.align 4, 0
	gdt:
		# Entry 0
		.hword 0 # Segment Limit 
		.hword 0 # Base Address
		.hword 0 # Bade Address cont. + type
		.hword 0 # Base Address cont. + Segment Limit cont. + type cont.

		# Code Segment (cs) entry
		.hword 0xffff
		.hword 0x0000
		.hword ((0b10011010  << 8) | 0b00000000 )
			#P DPL S Type(1 CRA) base address cont.
		.hword ((0b00000000 << 8 ) | 0b11001111 )
			#base address cont.    G D ? ? segment limit cont.

		# Data Segment (ds) entry
		.hword 0xffff
		.hword 0x0000
		.hword ((0b10010010 << 8) | 0b00000000 )
			# P DPL S Type(0 EWA) base address cont.
		.hword ((0b00000000 << 8) | 0b11001111 )
			# base address cont.  G D/B ? ? segment limit cont.

# We need to:
# 1. Take care of interrupts X ( we just disable them for now )
# 2. Take care of the stack X   ( easy? )
# 3. Take care of the gdt	( needed )
# 4. Take care of paging X ( we just identity map the first 2mb ) ( optional ? )
.section .text
	.extern main
.code32
_start:
	cli # disable interrupts
	cld # clear direction

load_gdt:
	# Set up gdt
	lgdt [gdtr]

	# Load segment registers
	mov ecx, 0x10
	mov ss, ecx
	mov ds, ecx
	mov es, ecx
	mov gs, ecx
	mov fs, ecx
	ljmp 0x8, finish_gdt
finish_gdt: # Only the finest gdt finland can offer

	# Set up stack
	mov esp, OFFSET kernel_stack+kernel_stack_size-1

	# Save multiboot values, these will also be the arguments to the main function
	push ebx
	push eax

setup_paging:
	# According to the amd64 manual, section 5.2.3, enteries are 8 bytes long, with PAE
	# Level 1 is the lowest

	# All 4*1 gb page directories
	DEFAULT_L3_ENTRY = 1 # Just the present bit(bit 0), *NOT* the cache disable bit(bit 4) and *NOT* the write through bit(bit 3) 

	mov eax, DEFAULT_L3_ENTRY	# According to the AMD64 manual section 5.4, only the address bits *above* bit 11 are stored in the address field of the entery specifed at section 5.2 page 139
	or eax, OFFSET l2_pt_1
	mov dword ptr [l3_pt], eax

	mov eax, DEFAULT_L3_ENTRY
	or eax, OFFSET l2_pt_2
	mov dword ptr [l3_pt+8], eax

	mov eax, DEFAULT_L3_ENTRY
	or eax, OFFSET l2_pt_3
	mov dword ptr [l3_pt+16], eax

	mov eax, DEFAULT_L3_ENTRY
	or eax, OFFSET l2_pt_4
	mov dword ptr [l3_pt+24], eax

	# Here, since for now we emulate paging in the actual os we can just give access to the pages to everybody as far as the cpu is concerned

	# All 512, 2mb pages
	DEFAULT_L2_ENTRY = (1 | (1 << 1) | (1 << 2) | (1 << 7) ) # The present bit(bit 0), the r/w bit(bit 1), the user access bit(bit 2), *NOT* the write through bit(bit 3), *NOT* the page cache disable bit(bit 4), bit 5 is set by the cpu it indicates access, bit 6 must be ignored, bit 7 indicates big pages and since we use 2mb pages it's 1, bit 8 must be ignored, bits 11-9 are available for os use
	mov ebx, 0
	l2_loop:
		mov eax, DEFAULT_L2_ENTRY
		shl ebx, 21
		or eax, ebx
		shr ebx, 18
		mov dword ptr [l2_pt_1+ebx], eax
		add eax, 0x40000000
		mov dword ptr [l2_pt_2+ebx], eax
		add eax, 0x40000000
		mov dword ptr [l2_pt_3+ebx], eax
		add eax, 0x40000000
		mov dword ptr [l2_pt_4+ebx], eax
		shr ebx, 3
		inc ebx
		cmp ebx, 512
	jl l2_loop

	# Level 1 table ( page table ), are not used in legacy 2mb PAE paging

enable_pae:
    # Enable Physical Address Extension
    mov eax, cr4
    or eax, 1 << 5
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
	cli
	hlt