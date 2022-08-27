fn main() {
    cc::Build::new()
        .file("src/asm_init_2mb_paging_long_mode_uefi.s")
        .compile("init_asm");
    for arg in &[
        "-ffreestanding", 
        "-nostartfiles", // no crt0
        "-nodefaultlibs",  // no c default libs
        "-static", // all static because kernels cannot easily be loaded dynamically
        "-Tlink.x", // linker script
        ] {
        println!("cargo:rustc-link-arg={}", arg);
    }
}
