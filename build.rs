fn main() {
    cc::Build::new()
        .file("src/asm_init_2mb_paging.s")
        .compile("init_asm");
    for arg in &[
        "-ffreestanding",
        "-nostartfiles",
        "-nodefaultlibs",
        "-static",
        "-Tlink.x",
        "-m32",
    ] {
        println!("cargo:rustc-link-arg={}", arg);
    }
}
