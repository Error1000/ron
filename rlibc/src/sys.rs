// On architectures where syscall is not supported this is an unused import
#[allow(unused_imports)]
use core::arch::asm;

pub const STDIN_FILENO: usize = 0;
pub const STDOUT_FILENO: usize = 1;
pub const STDERR_FILENO: usize = 2;

pub const O_RDONLY: usize = 0b00001;
pub const O_WRONLY: usize = 0b00010;
pub const O_RDWR: usize = O_RDONLY | O_WRONLY;
pub const O_APPEND: usize = 0b00100;
pub const O_CREAT: usize = 0b01000;
pub const O_TRUNC: usize = 0b10000;

pub const SEEK_CUR: usize = 0;
pub const SEEK_SET: usize = 1;
pub const SEEK_END: usize = 2;

#[no_mangle]
pub unsafe extern "C" fn open(pathname: *const u8, flags: core::ffi::c_int) -> core::ffi::c_int {
    load_syscall_argument_1(pathname as usize);
    load_syscall_argument_2(flags as usize);
    syscall(SyscallNumber::Open);
    read_syscall_return() as core::ffi::c_int
}

#[no_mangle]
pub unsafe extern "C" fn close(fd: core::ffi::c_int) -> core::ffi::c_int {
    load_syscall_argument_1(fd as usize);
    syscall(SyscallNumber::Close);
    read_syscall_return() as core::ffi::c_int
}

#[no_mangle]
pub unsafe extern "C" fn write(fd: core::ffi::c_int, buf: *const u8, count: core::ffi::c_size_t) -> core::ffi::c_ssize_t {
    load_syscall_argument_1(fd as usize);
    load_syscall_argument_2(buf as usize);
    load_syscall_argument_3(count as usize);
    syscall(SyscallNumber::Write);
    read_syscall_return() as core::ffi::c_ssize_t
}

#[no_mangle]
pub unsafe extern "C" fn read(fd: core::ffi::c_int, buf: *mut u8, count: core::ffi::c_size_t) -> core::ffi::c_ssize_t {
    load_syscall_argument_1(fd as usize);
    load_syscall_argument_2(buf as usize);
    load_syscall_argument_3(count as usize);
    syscall(SyscallNumber::Read);
    read_syscall_return() as core::ffi::c_ssize_t
}

#[no_mangle]
pub unsafe extern "C" fn lseek(fd: core::ffi::c_int, offset: core::ffi::c_long, whence: core::ffi::c_int) -> core::ffi::c_long {
    load_syscall_argument_1(fd as usize);
    load_syscall_argument_2(offset as usize);
    load_syscall_argument_3(whence as usize);
    syscall(SyscallNumber::LSeek);
    read_syscall_return() as core::ffi::c_long
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: core::ffi::c_size_t) -> *mut u8 {
    load_syscall_argument_1(size as usize);
    syscall(SyscallNumber::Malloc);
    read_syscall_return() as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    load_syscall_argument_1(ptr as usize);
    syscall(SyscallNumber::Free)
}

#[no_mangle]
pub unsafe extern "C" fn exit(code: core::ffi::c_int) {
    load_syscall_argument_1(code as usize);
    syscall(SyscallNumber::Exit);
}

#[repr(usize)]
pub enum SyscallNumber {
    Exit = 0,
    Read = 1,
    Write = 2,
    Open = 3,
    Close = 4,
    LSeek = 5,
    Malloc = 6,
    Free = 7,
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
        // SAFETY: SyscallNumber is reper(usize), value is usize
        // and we just checked that value is less than the max value of SyscallNumber
    }
}

// Architecture dependent definitions

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

#[cfg(target_arch = "riscv64")]
#[inline(always)]
pub unsafe fn read_argc() -> core::ffi::c_int {
    let value: usize;
    asm!("", out("a0") value);
    value as core::ffi::c_int
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
pub unsafe fn read_argv() -> *const *const core::ffi::c_char {
    let value: usize;
    asm!("", out("a1") value);
    value as *const *const core::ffi::c_char
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
pub unsafe fn setup_general_pointer() {
    asm!("la gp, __global_pointer$");
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

#[cfg(not(target_arch = "riscv64"))]
pub unsafe fn read_argc() -> core::ffi::c_int {
    unimplemented!("No argc reading function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
pub unsafe fn read_argv() -> *const *const core::ffi::c_char {
    unimplemented!("No argv reading function defined in c library for your architecture!");
}

#[cfg(not(target_arch = "riscv64"))]
#[inline(always)]
pub unsafe fn setup_general_pointer() {
    unimplemented!("No gp setup function defined in c library for your architecture!");
}
