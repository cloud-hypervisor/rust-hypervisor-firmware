fn main() {
    println!("cargo:rerun-if-changed=aarch64-unknown-none.json");
    println!("cargo:rerun-if-changed=aarch64-unknown-none.ld");
    println!("cargo:rerun-if-changed=x86_64-unknown-none.json");
    println!("cargo:rerun-if-changed=x86_64-unknown-none.ld");
}
