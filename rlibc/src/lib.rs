#![no_std]
#![feature(c_size_t)]

use core::ptr::null_mut;

pub mod cstr;
pub mod mem;
pub mod sys;

use sys::lseek;

use crate::{
    cstr::strlen,
    sys::{close, free, malloc, open, read, write, O_APPEND, O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY},
};

// TODO list:
// strcat *(done)*
// strncpy
// strcpy *(done)*
// strtok
// strstr *(done)*
// putchar
// getc
// getchar
// realloc
// isspace *(done)*
// isdigit *(done)*

// printf
// fprintf



#[cfg(not(feature = "nostartfiles"))]
#[panic_handler]
fn panic(_: &::core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(not(feature = "nostartfiles"))]
#[no_mangle]
pub unsafe extern "C" fn _start() {
    use crate::sys::{read_argc, read_argv, setup_general_pointer};
    use sys::exit;

    setup_general_pointer();

    exit(main(read_argc(), read_argv()));
    loop {}
}

#[cfg(not(feature = "nostartfiles"))]
extern "C" {
    pub fn main(argc: core::ffi::c_int, argv: *const *const core::ffi::c_char) -> core::ffi::c_int;
}

#[no_mangle]
pub unsafe extern "C" fn puts(str: *const core::ffi::c_char) -> core::ffi::c_int {
    let mut t = 0;
    let res = write(sys::STDOUT_FILENO as core::ffi::c_int, str, strlen(str) as core::ffi::c_size_t);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    let res = write(sys::STDOUT_FILENO as core::ffi::c_int, (&"\n").as_ptr() as *const core::ffi::c_char, 1);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    t as core::ffi::c_int
}

#[no_mangle]
pub unsafe extern "C" fn perror(str: *const core::ffi::c_char) -> core::ffi::c_int {
    let mut t = 0;
    let res = write(sys::STDERR_FILENO as core::ffi::c_int, str, strlen(str) as core::ffi::c_size_t);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    let res = write(sys::STDERR_FILENO as core::ffi::c_int, (&"\n").as_ptr() as *const core::ffi::c_char, 1);
    if res < 0 {
        return res as core::ffi::c_int;
    } else {
        t += res;
    }
    t as core::ffi::c_int
}

#[repr(C)]
pub struct FILE {
    fileno: core::ffi::c_int,
}

#[no_mangle]
pub unsafe extern "C" fn fopen(filename: *const core::ffi::c_char, mode: *const core::ffi::c_char) -> *mut FILE {
    let mode = core::ffi::CStr::from_ptr(mode as *const i8);
    let mode = if let Ok(val) = mode.to_str() {
        val
    } else {
        return null_mut();
    };

    // Transform flags into unix open options and extra options on the FILE struct if necessary
    // FIXME: Ensure that this is standards compliant
    let flags = match (mode.chars().nth(0), mode.chars().nth(1) == Some('+')) {
        (Some('r'), false) => O_RDONLY,
        (Some('w'), false) => O_WRONLY | O_CREAT | O_TRUNC,
        (Some('a'), false) => O_WRONLY | O_APPEND | O_CREAT,

        (Some('r'), true) => O_RDWR,
        (Some('w'), true) => O_RDWR | O_CREAT | O_TRUNC,
        (Some('a'), true) => O_RDWR | O_APPEND | O_CREAT,
        _ => return null_mut(),
    };

    let fd = open(filename, flags as core::ffi::c_int);
    if fd < 0 {
        return null_mut();
    }

    let file_ptr = malloc(core::mem::size_of::<FILE>()) as *mut FILE;
    if file_ptr.is_null() {
        return null_mut();
    }
    *file_ptr = FILE { fileno: fd };
    return file_ptr;
}

#[no_mangle]
pub unsafe extern "C" fn fclose(f: *mut FILE) -> core::ffi::c_int {
    if close((*f).fileno) < 0 {
        return -1;
    }
    free(f as *mut _);
    0
}

#[no_mangle]
pub unsafe extern "C" fn fwrite(
    buf: *const core::ffi::c_char,
    size: core::ffi::c_size_t,
    count: core::ffi::c_size_t,
    f: *mut FILE,
) -> core::ffi::c_size_t {
    let bytes = size * count;
    if bytes == 0 {
        return 0;
    }
    let res = write((*f).fileno, buf, bytes);
    if res < 0 {
        return 0;
    }
    (res as core::ffi::c_size_t) / size
}

#[no_mangle]
pub unsafe extern "C" fn fread(
    buf: *mut core::ffi::c_char,
    size: core::ffi::c_size_t,
    count: core::ffi::c_size_t,
    f: *mut FILE,
) -> core::ffi::c_size_t {
    let bytes = size * count;
    if bytes == 0 {
        return 0;
    }

    let res = read((*f).fileno, buf, bytes);
    if res < 0 {
        return 0;
    }
    (res as core::ffi::c_size_t) / size
}

#[no_mangle]
pub unsafe extern "C" fn fseek(f: *mut FILE, offset: core::ffi::c_long, origin: core::ffi::c_int) -> core::ffi::c_int {
    if lseek(unsafe { &*f }.fileno, offset, origin) > 0 {
        return 0;
    } else {
        return -1;
    }
}
