.intel_syntax noprefix
.global _start

kernel_stack_size = 4096
.section .bss
	.lcomm kernel_stack, kernel_stack_size
	# tables must be page aligned
	# Modes that we are interested in: Protected/Compatibility, 64-bit(long mode), 
	# This is with PAE, 4-level, that is PAE with long mode
	# Without PAE we can't have long mode and in compatiblity/protected mode it is only 2-level paging without PAE
	# 512 gb per level 3 table
      
    # According to the AMD64 programmer's manual vol. 2, figure 5-16 the pml4 has to be 4kb(one smallest sized page) aligned (the bottom 12 bits mustbe unused for address as they are used by the process context identifier)
	.balign 512*8
	l4_pt:
	.fill 512, 8, 0
	
	# 512 gb per level 3 table
	.balign 512*8
	l3_pt_1:
	.fill 512, 8, 0

.section .text
	.extern main
.code64
_start:
	cli # disable interrupts
	cld # clear direction

	# Set up stack
	mov rsp, OFFSET kernel_stack+kernel_stack_size-512
	
	# Save multiboot values, these will also be the arguments to the main function
	push rbx
	push rax

# For now only 512gb mapped
setup_paging:
	DEFAULT_L4_ENTRY = 0b111

    mov rax, OFFSET l3_pt_1 # address is 12 bit alignd ( the last 12 bits will be 0's anyways so no point in shifing it to the left and then back)
	or ax, DEFAULT_L4_ENTRY
	mov dword [l4_pt-4], rax

	DEFAULT_L3_ENTRY = (1 << 0 | 1 << 1 | 1 << 2 | 1 << 7) # Present (bit 0), r/w(bit 1), user(bit 2), page size 1gb ( bit 7)

	mov rbx, 0
	l3_loop:
		shl rbx, 30
		mov rax, rbx
		or ax, DEFAULT_L3_ENTRY
		shr rbx, 27

		mov dword [l3_pt_1+rbx-4], rax 

		shr rbx, 3
		inc rbx
		cmp rbx, 512
	jl l3_loop

enable_paging:
# FIXME: Crashes on real hardware
	# Put a pointer to the Page-Directory-Pointer(a.k.a l4_pt) into cr3
#	mov rax, OFFSET l4_pt
	# PCD(Page-Level Cache Disable)/ bit 4 = 0, we want cache :)
	# PWT(Page-Level Write Through)/ bit 3 = 0, a.k.a writeback policy which means on unexpected system shutdown, if the caches do not get flushed info might be lost
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
