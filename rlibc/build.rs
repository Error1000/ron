fn main() {
    for arg in &[
        "-nostartfiles",  // no crt0
        "-nostdlib",      // no default libc
    ] {
        println!("cargo:rustc-link-arg={}", arg);
    }
}
