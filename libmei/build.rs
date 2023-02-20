fn main() {
    let slab_mod = "src/vm/slab";
    let slab_rs = &format!("{slab_mod}.rs");
    let slab_cc = &format!("{slab_mod}.cc");
    let slab_hpp = &format!("{slab_mod}.hpp");

    set_compiler(&mut cxx_build::bridge(slab_rs))
        .file(slab_cc)
        .flag("-std=c++20")
        .flag("-ffreestanding")
        .flag("-mgeneral-regs-only")
        .flag("-fPIC")
        .flag("-fno-exceptions")
        .flag("-fno-unwind-tables")
        .flag("-fno-rtti")
        .flag("-fno-threadsafe-statics")
        .flag("-Wall")
        .flag("-Wextra")
        .cpp_link_stdlib(None)
        .compile("slab_cpp");

    println!("cargo:rerun-if-changed={slab_rs}");
    println!("cargo:rerun-if-changed={slab_cc}");
    println!("cargo:rerun-if-changed={slab_hpp}");
}

fn set_compiler(build: &mut cc::Build) -> &mut cc::Build {
    let target = std::env::var("TARGET").unwrap();
    let host = std::env::var("HOST").unwrap();

    const ARM_GCC_COMPILER: &str = "aarch64-none-elf-c++";

    if target != host {
        if let Ok(gcc_dir) = std::env::var("ARM_GCC_TOOLCHAIN_PATH") {
            build.compiler(format!("{gcc_dir}/{ARM_GCC_COMPILER}"));
        } else {
            build.compiler(ARM_GCC_COMPILER);
        }
    }

    build
}
