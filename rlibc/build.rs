fn main() {
    for arg in &[
        "-nostdlib", // no default libc
    ] {
        println!("cargo:rustc-link-arg={}", arg);
    }
}
