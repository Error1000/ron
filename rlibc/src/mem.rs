#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: core::ffi::c_size_t) -> *mut u8 {
    if n < core::mem::size_of::<usize>() {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
        return dest;
    }

    let dest_size = dest as *mut usize;
    let src_size = src as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();

    for i in 0..n_size {
        *dest_size.add(i) = *src_size.add(i);
    }

    for i in n_size * core::mem::size_of::<usize>()..n {
        *dest.add(i) = *src.add(i);
    }

    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(ptr1: *const u8, ptr2: *const u8, n: core::ffi::c_size_t) -> core::ffi::c_int {
    let ptr1_size = ptr1 as *mut usize;
    let ptr2_size = ptr2 as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();
    let mut ineq_i = None;

    for i in 0..n_size {
        if *ptr1_size.add(i) != *ptr2_size.add(i) {
            for subi in i * core::mem::size_of::<usize>()..(i + 1) * core::mem::size_of::<usize>() {
                if *ptr1.add(subi) != *ptr2.add(subi) {
                    ineq_i = Some(subi);
                    break;
                }
            }
            break;
        }
    }

    if ineq_i.is_none() {
        for i in n_size * core::mem::size_of::<usize>()..n {
            if *ptr1.add(i) != *ptr2.add(i) {
                ineq_i = Some(i);
                break;
            }
        }
    }

    if let Some(ineq_indx) = ineq_i {
        return *ptr1.add(ineq_indx) as i32 - *ptr2.add(ineq_indx) as i32;
    } else {
        return 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: core::ffi::c_int, n: core::ffi::c_size_t) -> *mut u8 {
    let c = c as u8;
    if n < core::mem::size_of::<usize>() {
        for i in 0..n {
            *dest.add(i) = c;
        }
        return dest;
    }
    let dest_size = dest as *mut usize;
    let n_size = n / core::mem::size_of::<usize>();
    // NOTE: Don't use from_ne_bytes as it causes a call to memset (don't know if directly or indirectly), causing recursion, leading to a stack overflow
    // Endianness doesn't matter because we just need to repeat a byte
    let mut c_size = 0usize;
    for i in 0..core::mem::size_of::<usize>() {
        c_size |= (c as usize) << (i * 8);
    }

    for i in 0..n_size {
        *(dest_size.add(i)) = c_size;
    }

    for i in n_size * core::mem::size_of::<usize>()..n {
        *(dest.add(i)) = c;
    }

    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(ptr1: *const u8, ptr2: *const u8, n: core::ffi::c_size_t) -> core::ffi::c_int {
    memcmp(ptr1, ptr2, n)
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: core::ffi::c_size_t) -> *mut u8 {
    if (dest as *const u8) == src {
        return dest;
    }

    let src_range = src..src.add(n);
    let has_overlap = src_range.contains(&(dest as *const u8)) || src_range.contains(&(dest.add(n) as *const u8));
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
