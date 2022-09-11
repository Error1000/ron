#![no_std]

// On architectures where syscall is not supported this is an unused import
#[allow(unused_imports)]
use core::arch::asm;

#[cfg(feature = "used-as-system-library")]
#[panic_handler]
fn panic(_: &::core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(feature = "used-as-system-library")]
#[no_mangle]
pub unsafe extern "C" fn _start() {
    exit(main() as isize);
    loop {}
}

#[cfg(feature = "used-as-system-library")]
extern "C" {
    pub fn main() -> core::ffi::c_int;
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
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
pub unsafe extern "C" fn memcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
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
pub unsafe extern "C" fn memset(dest: *mut u8, c: isize, n: usize) -> *mut u8 {
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
    // Endianness dosen't matter because we just need to repeat a byte
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
pub unsafe extern "C" fn bcmp(ptr1: *const u8, ptr2: *const u8, n: usize) -> i32 {
    memcmp(ptr1, ptr2, n)
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
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

#[no_mangle]
pub unsafe extern "C" fn write(fd: usize, buf: *const u8, count: usize) -> i32 {
    load_syscall_argument_1(fd);
    load_syscall_argument_2(buf as usize);
    load_syscall_argument_3(count);
    syscall(SyscallNumber::Write);
    read_syscall_return() as i32
}

#[no_mangle]
pub unsafe extern "C" fn exit(code: isize) {
    load_syscall_argument_1(code as usize);
    syscall(SyscallNumber::Exit);
}

#[repr(usize)]
pub enum SyscallNumber {
    Exit = 0,
    Write = 1,
    MaxValue,
}

impl TryFrom<usize> for SyscallNumber {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value >= SyscallNumber::MaxValue as usize {
            return Err(());
        } else {
            return Ok(unsafe { core::mem::transmute(value) });
        }
        // SAFTEY: SyscallNumber is reper(usize), value is usize
        // and we just checked that value is less than the max value of SyscallNumber
    }
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
unsafe fn load_syscall_argument_1(value: usize) {
    // NOTE: Uses linux abi
    asm!("", in("a0") value);
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
unsafe fn load_syscall_argument_2(value: usize) {
    // NOTE: Uses linux abi
    asm!("", in("a1") value);
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
unsafe fn load_syscall_argument_3(value: usize) {
    // NOTE: Uses linux abi
    asm!("", in("a2") value);
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
unsafe fn read_syscall_return() -> usize {
    // NOTE: Uses linux abi
    let value: usize;
    asm!("", out("a0") value);
    value
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
unsafe fn syscall(number: SyscallNumber) {
    // NOTE: Uses linux abi
    asm!("", in("a7") number as usize);
    asm!("ecall");
}

#[cfg(not(target_arch = "riscv64"))]
unsafe fn syscall(_number: SyscallNumber) {
    unimplemented!("No syscall function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
unsafe fn load_syscall_argument_1(_value: usize) {
    unimplemented!("No syscall argument 1 loading function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
unsafe fn load_syscall_argument_2(_value: usize) {
    unimplemented!("No syscall argument 2 loading function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
unsafe fn load_syscall_argument_3(_value: usize) {
    unimplemented!("No syscall argument 3 loading function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
unsafe fn read_syscall_return() -> usize {
    unimplemented!("No syscall return reading function defined in c library for your architecture!");
}
