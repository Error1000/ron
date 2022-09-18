use core::cmp::min;

use crate::mem::memcmp;

#[no_mangle]
pub unsafe extern "C" fn strlen(str: *const u8) -> core::ffi::c_ulong {
    let mut size: core::ffi::c_ulong = 0;
    while unsafe { *str.add(size as usize) } != b'\0' {
        size += 1;
    }
    size
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(str1: *const u8, str2: *const u8) -> core::ffi::c_int {
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
