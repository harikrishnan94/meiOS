// build.rs

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let profile = env::var("PROFILE").unwrap();
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("profile");
    fs::write(dest_path, profile).unwrap();

    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo:rerun-if-changed=mei-aarch64.ld");
    println!("cargo:rerun-if-changed=aarch64-unknown-none-softfloat.json");
}
