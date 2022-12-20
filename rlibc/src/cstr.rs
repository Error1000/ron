use core::{cmp::min, ptr::null_mut};

use crate::mem::{memcmp, memcpy, memset};

#[no_mangle]
pub unsafe extern "C" fn strchr(str: *const core::ffi::c_char, ch: core::ffi::c_int) -> *mut core::ffi::c_char {
    let ch: core::ffi::c_char = ch as core::ffi::c_char;
    // Finds the first occurrence of ch (after conversion to char as if by (char)ch) in the null-terminated byte string pointed to by str (each character interpreted as unsigned char).
    // The terminating null character is considered to be a part of the string and can be found when searching for '\0'.
    // The behavior is undefined if str is not a pointer to a null-terminated byte string
    // Return value: Pointer to the found character in str, or null pointer if no such character is found. 
    // Source: https://en.cppreference.com/w/c/string/byte/strchr
    for i in 0..=strlen(str) {
        if *str.add(i as usize) == ch {
            return str.add(i as usize) as *mut core::ffi::c_char;
        }
    }
    return null_mut();
}


// Strtok is specifically *not* thread safe, so modifying a global without synchronization is fine
static mut STRTOK_STR: *mut core::ffi::c_char = null_mut();

#[no_mangle]
pub unsafe extern "C" fn strtok(mut str: *mut core::ffi::c_char, delim: *const core::ffi::c_char) -> *mut core::ffi::c_char {
    // Returns: Pointer to the beginning of the next token or a nullptr if there are no more tokens. 

    if str == null_mut() {
        if STRTOK_STR != null_mut() {
            str = STRTOK_STR;
        }else{
            // Both STRTOK_STR is null and str is null
            // This means that your first call to strtok was with str == NULL
            return null_mut();
        }
    }

    // If str is not a null pointer, the call is treated as the first call to strtok for this particular string.
    let str_len = strlen(str) as usize;
    let delim_len = strlen(delim) as usize;

    // Find first non-delimiter char
    let mut token_start = None;
    for i in 0..str_len {
        let mut is_not_inc = true;
        for j in 0..delim_len {
            if *str.add(i) == *delim.add(j) {
                is_not_inc = false;
                break;
            } 
        }
        if is_not_inc { token_start = Some(i); break; }
    }

    let token_start = if let Some(val) = token_start { val } 
    else { 
        /* If no such character was found, there are no tokens in str at all, 
        and the function returns a null pointer. */ 
        return null_mut(); 
    };
        
    // searches from that point on for the first character that is contained in delim. 
    let mut token_end = None;
    for i in token_start..str_len {
        let mut is_inc = false;
        for j in 0..delim_len {
            if *str.add(i) == *delim.add(j) {
                is_inc = true;
                break;
            }
        }
        if is_inc { token_end = Some(i); break; }
    }

    // token_end "points" to the delimiter char after the end of the token
    let token_end = if let Some(val) = token_end { val } 
    else { 
        /* If no such character was found, str has only one token, 
              and the future calls to strtok will return a null pointer */ 
        STRTOK_STR = null_mut(); 
        return str.add(token_start); 
    };

    *str.add(token_end) = '\0' as core::ffi::c_char;
    STRTOK_STR = str.add(token_end+1);

    return str.add(token_start);
}


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

    // NOTE:
    // "If substr points to an empty string, str is returned."
    // This is true because memcmp with a length of 0 will return 0, so it will return str.add(0) because i starts at 0.

    let str_len = strlen(str);
    let substr_len = strlen(substr);
    for i in 0..str_len {
        // i + substr_len - 1 is the offset of the last byte accessed by memcmp
        if i + substr_len - 1 >= str_len { break; }
        if memcmp(str.add(i as usize), substr, substr_len as usize) == 0 {
            // Const casting is fun :)
            return unsafe{str.add(i as usize)} as *mut core::ffi::c_char;
        }
    }
    
    return null_mut();
}

#[no_mangle]
pub unsafe extern "C" fn strncpy(dest: *mut core::ffi::c_char, src: *const core::ffi::c_char, count: core::ffi::c_size_t) -> *mut core::ffi::c_char {
    // Copies at most count characters of the character array pointed to by src (including the terminating null character, but not any of the characters that follow the null character) to character array pointed to by dest.
    // If count is reached before the entire array src was copied, the resulting character array is not null-terminated.
    // If, after copying the terminating null character from src, count is not reached, additional null characters are written to dest until the total of count characters have been written.
    // The behavior is undefined if the character arrays overlap, if either dest or src is not a pointer to a character array (including if dest or src is a null pointer), if the size of the array pointed to by dest is less than count, or if the size of the array pointed to by src is less than count and it does not contain a null character.
    // returns a copy of dest
    // Source: https://en.cppreference.com/w/c/string/byte/strncpy
    let src_len = strlen(src) as usize + 1 /* include null-terminator */;
    if src_len >= count {
        memcpy(dest, src, count);
    }else{ // src_len < count <=> count > src_len <=> count-src_len > 0
        memcpy(dest, src, src_len);
        // If, after copying the terminating null character from src, count is not reached, additional null characters are written to dest until the total of count characters have been written.
        memset(dest.add(src_len)/* points after the data we just memcpy-ed */, 0, count-src_len);
    }
    
    return dest;
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
    match ch as u8 {
        b'0' | b'1' | b'2' | b'3' | b'4'| b'5' | b'6' | b'7' | b'8' | b'9' => return 1,
        _ => return 0
    }
}
