const MULTIBOOT2_MAGIC: u32 = 0xE85250D6;
const MULTIBOOT2_ARCH: u32 = 0x0;
const MULTIBOOT2_HEADER_LEN_IN_U32S: usize = 8;

#[no_mangle]
pub static multiboot_header: [u32; MULTIBOOT2_HEADER_LEN_IN_U32S] = [
    MULTIBOOT2_MAGIC,
    MULTIBOOT2_ARCH,
    (MULTIBOOT2_HEADER_LEN_IN_U32S * core::mem::size_of::<u32>()) as u32,
    (-((MULTIBOOT2_MAGIC
        + MULTIBOOT2_ARCH
        + (MULTIBOOT2_HEADER_LEN_IN_U32S * core::mem::size_of::<u32>()) as u32) as i32)) as u32,
    0x7,
    0x8,
    // End tags
    0x0,
    0x8,
];

// Techincally dosen't initialise anything, as much as it just makes sure the needed structures are kept in the compiler binary
// (a.k.a not optimised away by the compiler because they are not used)
pub fn init(r1: usize, r2: usize) -> &'static [u32] {
    // Why yes rust i do need the statics i declared, please don't optimise them out of the executable
    core::hint::black_box(core::convert::identity(multiboot_header.as_ptr()));
    if r1 != 0x36d76289 {
        panic!("Kernel was not booted by a multiboot-compatible bootloader! ")
    }
    unsafe {
        core::slice::from_raw_parts(
            core::mem::transmute::<_, *const u32>(r2).offset(2),
            (*core::mem::transmute::<_, *const u32>(r2) as usize) / core::mem::size_of::<u32>(),
        )
    }
}
