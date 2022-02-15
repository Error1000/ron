use core::{mem, slice};


const MULTIBOOT_MAGIC: u32 = 0x1BADB002;
const MULTIBOOT_FLAGS: u32 = 0x0; 
const MULTIBOOT_HEADER_LEN_IN_U32S: usize = 3;

#[no_mangle]
pub static multiboot_header: [u32; MULTIBOOT_HEADER_LEN_IN_U32S] = [
    MULTIBOOT_MAGIC,
    MULTIBOOT_FLAGS,
    (
     -(
        ( MULTIBOOT_MAGIC + MULTIBOOT_FLAGS ) as i32
      )
    ) as u32,
];

// Techincally dosen't initialise anything, as much as it just makes sure the needed structures are kept in the compiler binary 
// (a.k.a not optimised away by the compiler because they are not used)
pub fn init(r1: u32, _r2: u32) {
    // Why yes rust i do need the statics i declared, please don't optimise them out of the executable
    core::hint::black_box(core::convert::identity(multiboot_header.as_ptr()));
    if r1 != 0x2BADB002 { panic!("Kernel was not booted by a multiboot-compatible bootloader! ") }
}