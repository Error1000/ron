.intel_syntax noprefix
.global _start

kernel_stack_size = 1024*1024
.section .bss
	# NOTE for future helpless programmers: this needs to be aligned so that movups and movaps
	# don't general protection fault when used when the stack pointer is used
	# because the compiler assumes the stack pointer is aligned
	#
	# https://stackoverflow.com/questions/67243284/why-movaps-causes-segmentation-fault
	# From Intel® 64 and IA-32 architectures software developer’s manual, MOVAPS specification:
    # 	MOVAPS—Move *Aligned* Packed Single-Precision Floating-Point Values
    # 	When the source or destination operand is a memory operand, the operand must be aligned on a 16-byte (128-bit version), 32-byte (VEX.256 encoded version) or 64-byte (EVEX.512 encoded version) boundary or a general protection exception (#GP) will be generated.
	.balign 64
	.lcomm kernel_stack, kernel_stack_size

	# tables must be page aligned
	# Modes that we are interested in: Protected/Compatibility, 64-bit(long mode), 
	# This is with PAE, 4-level, that is PAE with long mode
	# Without PAE we can't have long mode and in compatiblity/protected mode it is only 2-level paging without PAE
	# 512 gb per level 3 table
      
    # According to the AMD64 programmer's manual vol. 2, figure 5-16 the pml4 has to be 4kb(one smallest sized page) aligned (the bottom 12 bits mustbe unused for address as they are used by the process context identifier)
	.balign 512*8
	l4_pt:
	.fill 1, 8, 0
	
	# 512 gb per level 3 table
	.balign 512*8
	l3_pt_1:
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

.section .text
	.extern main
.code64
_start:
	cli # disable interrupts
	cld # clear direction

	# Set up stack
	mov rsp, OFFSET kernel_stack+kernel_stack_size
	
	# Save multiboot values, these will also be the arguments to the main function
	push rbx
	push rax

    mov ecx, 0xC0000080          # Set the C-register to 0xC0000080, which is the EFER MSR.
    rdmsr                        # Read from the model-specific register.
    or eax, (1 << 8)               # Set the LM-bit which is the 9th bit (bit 8).
    wrmsr                        # Write to the model-specific register.


# For now only 4gb mapped
setup_paging:
	DEFAULT_L4_ENTRY = (1 << 0 | 1 << 1 | 1 << 2)

    mov rax, OFFSET l3_pt_1 # address is 12 bit alignd ( the last 12 bits will be 0's anyways so no point in shifing it to the left and then back)
	or ax, DEFAULT_L4_ENTRY
	mov dword [l4_pt-4], rax

	DEFAULT_L3_ENTRY = (1 << 0 | 1 << 1 | 1 << 2) # Present (bit 0), r/w(bit 1), user(bit 2), page size ( bit 7)
	mov rax, OFFSET l2_pt_1
	or ax, DEFAULT_L3_ENTRY
	mov dword [l3_pt_1-4+8*0], rax

	mov rax, OFFSET l2_pt_2
	or ax, DEFAULT_L3_ENTRY
	mov dword [l3_pt_1-4+8*1], rax

	mov rax, OFFSET l2_pt_3
	or ax, DEFAULT_L3_ENTRY
	mov dword [l3_pt_1-4+8*2], rax

	mov rax, OFFSET l2_pt_4
	or ax, DEFAULT_L3_ENTRY
	mov dword [l3_pt_1-4+8*3], rax

	DEFAULT_L2_ENTRY = (1 << 0 | 1 << 1 | 1 << 2 | 1 << 7)
	mov rbx, 0
	l2_loop:
		shl rbx, 21 # 2^21 bytes = 2 mb
		mov rax, rbx
		or ax, DEFAULT_L2_ENTRY
		shr rbx, 21-3

		mov dword [l2_pt_1+rbx-4], rax 
        
		add rax, 0x40000000 # 1 gb between l2 pages
		mov dword [l2_pt_2+rbx-4], rax 

		add rax, 0x40000000 # 1 gb between l2 pages
		mov dword [l2_pt_3+rbx-4], rax 

		add rax, 0x40000000 # 1 gb between l2 pages
		mov dword [l2_pt_4+rbx-4], rax 

		shr rbx, 3
		inc rbx
		cmp rbx, 512 # 512*2mb = 1024 mb = 1 gb
	jl l2_loop

enable_paging:	
	# Put a pointer to the Page-Directory-Pointer(a.k.a l4_pt) into cr3
	mov rax, OFFSET l4_pt
	# PCD(Page-Level Cache Disable)/ bit 4 = 0, we want cache :)
	# PWT(Page-Level Write Through)/ bit 3 = 0, a.k.a writeback policy which means on unexpected system shutdown, if the caches do not get flushed info might be lost
#	FIXME: Causes crash on real hardware, why?
#	mov cr3, rax

enable_simd:
	mov rax, cr0
	and ax, 0xFFFB	# ~(1<<2) # disable coprocessor(old floating point) emulation CR0.EM
	or ax, 0x2			    # enable coprocessor monitoring  CR0.MP
	mov cr0, rax

	mov rax, cr4
	or ax, 0b11 << 9		# enable CR4.OSFXSR and CR4.OSXMMEXCPT, enables simd and unmasked simd floating point exceptions
	mov cr4, rax

goto_kmain:
	pop rdi
	pop rsi
	call main
