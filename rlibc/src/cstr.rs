use core::{cmp::min, ffi::CStr, ptr::{null_mut, null}};

use crate::mem::{memcmp, memcpy};

#[no_mangle]
pub unsafe extern "C" fn strlen(str: *const core::ffi::c_char) -> core::ffi::c_ulong {
    let mut size: core::ffi::c_ulong = 0;
    while unsafe { *str.add(size as usize) } as u8 != b'\0' {
        size += 1;
    }
    size
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(str1: *const core::ffi::c_char, str2: *const core::ffi::c_char) -> core::ffi::c_int {
    let len_1 = strlen(str1);
    let len_2 = strlen(str2);
    // min(len_1, len_2) would be 0 in these cases which would cause memcmp to return 0, but we should not return 0, because the string are *NOT* equal
    if len_1 == 0 && len_2 != 0 {
        return -1;
    }
    if len_1 != 0 && len_2 == 0 {
        return -1;
    }

    memcmp(str1, str2, min(len_1, len_2) as usize)
}

#[no_mangle]
pub unsafe extern "C" fn strstr(str: *const core::ffi::c_char, substr: *const core::ffi::c_char) -> *mut core::ffi::c_char {
    // Finds the first occurrence of the null-terminated byte string pointed to by substr in the null-terminated byte string pointed to by str. 
    // The terminating null characters are not compared.
    // The behavior is undefined if either str or substr is not a pointer to a null-terminated byte string.
    // Returns: Pointer to the first character of the found substring in str, or a null pointer if such substring is not found. If substr points to an empty string, str is returned. 

    // FIXME: This would probably be easier to do with memcmp but whatever the compiler should be smart enough to optimize it anyway, maybe, idk

    if substr == null() { return str as *mut core::ffi::c_char; }
    let str = CStr::from_ptr(str);
    let substr = CStr::from_ptr(substr);

    let str = str.to_bytes(); // Does *not* include null byte
    let substr = substr.to_bytes(); // Does *not* include null byte
    for i in 0..str.len() {
        let mut included = true;
        for j in i..i+substr.len() {
           if j >= str.len() { included = false; break; }
           let sub_ind = j-i;
           if str[j] != substr[sub_ind] {
                included = false;
                break;
           } 
        }
        if included {
            // Const casting is fun :)
            return unsafe{str.as_ptr().add(i)} as *mut core::ffi::c_char;
        }
    }
    
    return null_mut();
}

#[no_mangle]
pub unsafe extern "C" fn strcpy(dest: *mut core::ffi::c_char, src: *const core::ffi::c_char) -> *mut core::ffi::c_char {
    // Copies the null-terminated byte string pointed to by src, including the null terminator, to the character array whose first element is pointed to by dest.
    // The behavior is undefined if the dest array is not large enough. The behavior is undefined if the strings overlap. The behavior is undefined if either dest is not a pointer to a character array or src is not a pointer to a null-terminated byte string.
    // Source: https://en.cppreference.com/w/c/string/byte/strcpy
    memcpy(dest, src, strlen(src) as usize + 1 /* also copy the null-terminator from the src string */)
}

#[no_mangle]
pub unsafe extern "C" fn strcat(dest: *mut core::ffi::c_char, src: *const core::ffi::c_char) -> *mut core::ffi::c_char {
    // Appends a copy of the null-terminated byte string pointed to by src to the end of the null-terminated byte string pointed to by dest. The character src[0] replaces the null terminator at the end of dest. The resulting byte string is null-terminated.
    // The behavior is undefined if the destination array is not large enough for the contents of both src and dest and the terminating null character. The behavior is undefined if the strings overlap. The behavior is undefined if either dest or src is not a pointer to a null-terminated byte string.
    // Source: https://en.cppreference.com/w/c/string/byte/strcat
    strcpy(dest.add(strlen(dest) as usize), src)
}

#[no_mangle]
pub unsafe extern "C" fn isspace(ch: core::ffi::c_int) -> core::ffi::c_int {
    // Checks if the given character is a whitespace character, i.e. 
    // either space (0x20), form feed (0x0c), line feed (0x0a), 
    // carriage return (0x0d), horizontal tab (0x09) or vertical tab (0x0b).
    // Non-zero value if the character is a whitespace character, zero otherwise. 
    // https://en.cppreference.com/w/c/string/byte/isspace
    match ch {
        0x20 | 0x0c | 0x0a | 0x0d | 0x09 | 0x0b => return 1,
        _ => return 0
    }
}

#[no_mangle]
pub unsafe extern "C" fn isdigit(ch: core::ffi::c_int) -> core::ffi::c_int {
    // Checks if the given character is a numeric character (0123456789). 
    // Non-zero value if the character is a numeric character, zero otherwise. 
    // The behavior is undefined if the value of ch is not representable as unsigned char and is not equal to EOF. 
    // https://en.cppreference.com/w/c/string/byte/isdigit

    // The behavior is undefined if the value of ch is not representable as unsigned char and is not equal to EOF. 
    // TODO: Wait so what happens if ch is EOF, right now we just return 0, which matches with linux, but is that standard?
    match ch as u8 {
        b'0' | b'1' | b'2' | b'3' | b'4'| b'5' | b'6' | b'7' | b'8' | b'9' => return 1,
        _ => return 0
    }
}
