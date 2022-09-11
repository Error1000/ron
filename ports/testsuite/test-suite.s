.section .text
	.global _start
_start:
	li x5, 0
	# div

	# 20/3 = 6
	li x1, 3
	li x2, 20
	div x3, x2, x1
	li x4, 6
	addi x5, x5, 1
	bne x3, x4, error

	# -20/-3 = 6
	li x1, -3
	li x2, -20
	div x3, x2, x1
	li x4, 6
	addi x5, x5, 1
	bne x3, x4, error

	# 20/-3 = -6
	li x1, -3
	li x2, 20
	div x3, x2, x1
	li x4, -6
	addi x5, x5, 1
	bne x3, x4, error

	# -20/3 = -6
	li x1, 3
	li x2, -20
	div x3, x2, x1
	li x4, -6
	addi x5, x5, 1
	bne x3, x4, error

	li x1, -1<<63
	li x2, -1<<63
	div x3, x2, x1
	li x4, 1
	addi x5, x5, 1
	bne x3, x4, error

	li x1, -1
	li x2, 0
	div x3, x2, x1
	li x4, 0
	addi x5, x5, 1
	bne x3, x4, error


	# divu
	li x1, 3
	li x2, 20
	divu x3, x2, x1
	li x4, 6
	addi x5, x5, 1
	bne x3, x4, error

        li x1, 3074457345618258599 # -3
        li x2, -20
        divu x3, x2, x1
        li x4, 6
	addi x5, x5, 1
        bne x3, x4, error

        li x1, -1<<63
        li x2, -1<<63
        divu x3, x2, x1
        li x4, 1
	addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, 0
        divu x3, x2, x1
        li x4, 0
	addi x5, x5, 1
        bne x3, x4, error

	# divuw
        li x1, 3
        li x2, 20
        divuw x3, x2, x1
        li x4, 6
	addi x5, x5, 1
        bne x3, x4, error

        li x1, 715827879
        li x2, -20 << 32 >> 32
        divuw x3, x2, x1
        li x4, 6
	addi x5, x5, 1
        bne x3, x4, error

        li x1, -1<<31
        li x2, -1<<31
        divuw x3, x2, x1
        li x4, 1
	addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, 0
        divuw x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	# divw
	li x1, 3
        li x2, 20
        divw x3, x2, x1
        li x4, 6
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -3
        li x2, -20
        divw x3, x2, x1
        li x4, 6
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -3
        li x2, 20
        divw x3, x2, x1
        li x4, -6
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 3
        li x2, -20
        divw x3, x2, x1
        li x4, -6
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -1<<31
        li x2, -1<<31
        divw x3, x2, x1
        li x4, 1
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, 0
        divw x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	# mul
        li x1, 0x6db6db6db6db6db7
        li x2, 0x0000000000007e00
        mul x3, x2, x1
        li x4, 0x0000000000001200
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x6db6db6db6db6db7
        li x2, 0x0000000000007fc0
        mul x3, x2, x1
        li x4, 0x0000000000001240
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000000
        li x2, 0x00000000
        mul x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000001
        li x2, 0x00000001
        mul x3, x2, x1
        li x4, 0x00000001
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000007
        li x2, 0x00000003
        mul x3, x2, x1
        li x4, 0x00000015
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0x0000000000000000
        mul x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000000
        li x2, 0xffffffff80000000
        mul x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0xffffffff80000000
        mul x3, x2, x1
        li x4, 0x0000400000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x000000000002fe7d
        li x2, 0xaaaaaaaaaaaaaaab
        mul x3, x2, x1
        li x4, 0x000000000000ff7f
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xaaaaaaaaaaaaaaab
        li x2, 0x000000000002fe7d
        mul x3, x2, x1
        li x4, 0x000000000000ff7f
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11
        li x2, 13
        mul x3, x2, x1
        li x4, 143
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11
        li x2, 14
        mul x3, x2, x1
        li x4, 154
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 13
        li x2, 13
        mul x3, x2, x1
        li x4, 169
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11
        li x2, 15
        mul x3, x2, x1
        li x4, 165
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 31
        li x2, 0
        mul x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 32
        mul x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 0
        mul x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	# mulw
	li x1, 0x00000000
	li x2, 0x00000000
	mulw x3, x2, x1
	li x4, 0x00000000
	addi x5, x5, 1
	bne x3, x4, error

        li x1, 0x00000001
        li x2, 0x00000001
        mulw x3, x2, x1
        li x4, 0x00000001
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000007
        li x2, 0x00000003
        mulw x3, x2, x1
        li x4, 0x00000015
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0x0000000000000000
        mulw x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000000
        li x2, 0xffffffff80000000
        mulw x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0xffffffff80000000
        mulw x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

	#mulh
        li x1, 0x00000000
        li x2, 0x00000000
        mulh x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000001
        li x2, 0x00000001
        mulh x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000007
        li x2, 0x00000003
        mulh x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0x0000000000000000
        mulh x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000000
        li x2, 0xffffffff80000000
        mulh x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0xffffffff80000000
        mulh x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11<<32
        li x2, 13<<32
        mulh x3, x2, x1
        li x4, 143
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11<<32
        li x2, 14<<32
        mulh x3, x2, x1
        li x4, 154
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 11<<32
        li x2, 15<<32
        mulh x3, x2, x1
        li x4, 165
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 31<<32
        li x2, 0
        mulh x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 32<<32
        mulh x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 0
        mulh x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	#mulhsu
        li x1, 0x00000000
        li x2, 0x00000000
        mulhsu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000001
        li x2, 0x00000001
        mulhsu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000007
        li x2, 0x00000003
        mulhsu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffffffff8000
        li x2, 0x0000000000000000
        mulhsu x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x0000000000000000
        li x2, 0xffffffff80000000
        mulhsu x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 13<<32
        li x2, 11<<32
        mulhsu x3, x2, x1
        li x4, 143
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 14<<32
        li x2, 11<<32
        mulhsu x3, x2, x1
        li x4, 154
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 15<<32
        li x2, 11<<32
        mulhsu x3, x2, x1
        li x4, 165
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 31<<32
        li x2, 0
        mulhsu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 32<<32
        mulhsu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 0
        mulhsu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	# mulhu
        li x1, 0x00000000
        li x2, 0x00000000
        mulhu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000001
        li x2, 0x00000001
        mulhu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x00000007
        li x2, 0x00000003
        mulhu x3, x2, x1
        li x4, 0x00000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x0000000000000000
        li x2, 0xffffffffffff8000
        mulhu x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffff80000000
        li x2, 0x00000000
        mulhu x3, x2, x1
        li x4, 0x0000000000000000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xffffffff80000000
        li x2, 0xffffffffffff8000
        mulhu x3, x2, x1
        li x4, 0xffffffff7fff8000
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0xaaaaaaaaaaaaaaab
        li x2, 0x000000000002fe7d
        mulhu x3, x2, x1
        li x4, 0x000000000001fefe
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0x000000000002fe7d
        li x2, 0xaaaaaaaaaaaaaaab
        mulhu x3, x2, x1
        li x4, 0x000000000001fefe
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 13<<32
        li x2, 11<<32
        mulhu x3, x2, x1
        li x4, 143
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 14<<32
        li x2, 11<<32
        mulhu x3, x2, x1
        li x4, 154
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 15<<32
        li x2, 11<<32
        mulhu x3, x2, x1
        li x4, 165
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 31<<32
        mulhu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 32<<32
        li x2, 0
        mulhu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 0
        li x2, 0
        mulhu x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

	# rem
	li x1, 6
        li x2, 20
        rem x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 6
        li x2, -20
        rem x3, x2, x1
        li x4, -2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, 20
        rem x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, -20
        rem x3, x2, x1
        li x4, -2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 1
        li x2, -1<<63
        rem x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, -1<<63
        rem x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        # remuw
	li x1, 6
        li x2, 20
        remuw x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 6
        li x2, -20
        remuw x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, 20
        remuw x3, x2, x1
        li x4, 20
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, -20
        remuw x3, x2, x1
        li x4, -20
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 1
        li x2, -1<<31
        remuw x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, -1<<31
        remuw x3, x2, x1
        li x4, -1<<31
        addi x5, x5, 1
        bne x3, x4, error

        # remw
	li x1, 6
        li x2, 20
        remw x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 6
        li x2, -20
        remw x3, x2, x1
        li x4, -2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, 20
        remw x3, x2, x1
        li x4, 2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -6
        li x2, -20
        remw x3, x2, x1
        li x4, -2
        addi x5, x5, 1
        bne x3, x4, error

        li x1, 1
        li x2, -1<<31
        remw x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error

        li x1, -1
        li x2, -1<<31
        remw x3, x2, x1
        li x4, 0
        addi x5, x5, 1
        bne x3, x4, error


        # c.addi4spn
        li sp, 0x1234
        c.addi4spn a0, sp, 1020
        li x4, 0x1234+1020
        addi x5, x5, 1
        bne a0, x4, error

        # c.addi16sp
        li sp, 0x1234
        c.addi16sp sp, 496
        li x4, 0x1234+496
        addi x5, x5, 1
        bne sp, x4, error

        c.addi16sp sp, -512
        li x4, 0x1234+496-512
        addi x5, x5, 1
        bne sp, x4, error

        j data_end
data: 
        .dword 0xfedcba9876543210 
        .dword 0xfedcba9876543210 
data_end:
        # mixed instructions
        la a1, data
        c.lw a0, 4(a1) 
        addi a0, a0, 1
        c.sw a0, 4(a1)
        c.lw a2, 4(a1)
        li x4, 0xfffffffffedcba99
        addi x5, x5, 1
        bne a2, x4, error

        c.ld a0, 0(a1) 
        addi a0, a0, 1
        c.sd a0, 0(a1)
        c.ld a2, 0(a1)
        li x4, 0xfedcba9976543211
        addi x5, x5, 1
        bne a2, x4, error

        j success
error:
	# exit(1);
	li a7, 0
	li a0, 1
	ecall

hlt2:	j hlt2

success:
	# exit(0);
	li a7, 0
	li a0, 0
	ecall
hlt1:   j hlt1
