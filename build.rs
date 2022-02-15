fn main() {
    cc::Build::new()
        .file("src/asm_init_2mb_paging.s")
        .compile("init_asm");
    for arg in &[
        "-ffreestanding", 
        "-nostartfiles", // no crt0
        "-nodefaultlibs",  // no c default libs
        "-static", // all static because kernels cannot easily be loaded dynamicly
        "-Tlink.x", // linker script
        "-m32", // set bit width
    ] {
        println!("cargo:rustc-link-arg={}", arg);
    }
}
