#![no_std]

use {core::panic::PanicInfo, core::arch::asm};

#[cfg(feature="used-as-library")]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if n < core::mem::size_of::<usize>() {
        for i in 0..n {
            *dest.offset(i as isize) = *src.offset(i as isize);
        }
        return dest;
    }
    let dest_size = dest as *mut usize;
    let src_size = src as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();

    for i in 0..n_size {
        *dest_size.offset(i as isize) = *src_size.offset(i as isize);
    }
    for i in n_size * core::mem::size_of::<usize>()..n {
        *dest.offset(i as isize) = *src.offset(i as isize);
    }
    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
    let ptr1_size = ptr1 as *mut usize;
    let ptr2_size = ptr2 as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();
    let mut ineq_i = 0;
    let mut eq: bool = true;
    for i in 0..n_size {
        if *ptr1_size.add(i) != *ptr2_size.add(i) {
            eq = false;
            for subi in i * core::mem::size_of::<usize>()..(i + 1) * core::mem::size_of::<usize>() {
                if *ptr1.add(i) != *ptr2.add(i) {
                    ineq_i = subi;
                    break;
                }
            }
            break;
        }
    }
    for i in n_size * core::mem::size_of::<usize>()..n {
        if *ptr1.offset(i as isize) != *ptr2.offset(i as isize) {
            eq = false;
            ineq_i = i;
            break;
        }
    }
    if eq {
        return 0;
    } else {
        return *ptr1.add(ineq_i) as i32 - *ptr2.add(ineq_i) as i32;
    }
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: isize, n: usize) -> *mut u8 {
    let c = c as u8;
    if n < core::mem::size_of::<usize>() {
        for i in 0..n {
            *dest.offset(i as isize) = c;
        }
        return dest;
    }
    let dest_size = dest as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();
    // NOTE: Don't use from_ne_bytes as it causes a call to memset (don't know if directly or indirectly), causing recursion, leading to a stack overflow
    // Endianness dosen't matter
    let mut c_size = 0usize;
    for i in 0..core::mem::size_of::<usize>() / core::mem::size_of::<u8>() {
        c_size |= (c as usize) << i;
    }
    for i in 0..n_size {
        *(dest_size.offset(i as isize)) = c_size;
    }
    for i in n_size * core::mem::size_of::<usize>()..n {
        *(dest.offset(i as isize)) = c;
    }
    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    memcmp(ptr1, ptr2, n)
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as *const u8) == src {
        return dest;
    }
    let src_range = src..src.add(n);
    let has_overlap =
        src_range.contains(&(dest as *const u8)) || src_range.contains(&(dest.add(n) as *const u8));
    if !has_overlap {
        memcpy(dest, src, n);
    } else if (dest as *const u8) < src {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
    } else if (dest as *const u8) > src {
        for i in (0..n).rev() {
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn exit(code: usize) {
    load_syscall_argument_1(code);
    syscall(1);
}


#[cfg(target_arch="riscv64")]
#[inline(always)]
unsafe fn load_syscall_argument_1(value: usize) {
    // NOTE: Uses linux abi
    asm!(
        "",
        in("a0") value,
    );
}

#[cfg(target_arch="riscv64")]
#[inline(always)]
unsafe fn syscall(number: usize) {
    // NOTE: Uses linux abi
    asm!(
        "",
        in("a7") number,
    );
    asm!(
        "ecall"
    );
}

#[cfg(not(target_arch="riscv64"))]
unsafe fn syscall(_number: usize) { unimplemented!("No syscall function defined in c library for your architecture!"); }

#[cfg(not(target_arch="riscv64"))]
unsafe fn load_syscall_argument_1(_value: usize) { unimplemented!("No syscall argument 1 loading function defined in c library for your architecture!"); }
