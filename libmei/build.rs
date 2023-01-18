fn main() {
    let ttt_rs = "src/mmu/translation_table.rs";
    let ttt_cxx = "src/mmu/translation_table.cpp";
    let out_dir = std::env::var("OUT_DIR").unwrap();

    cxx_build::bridge(ttt_rs)
        .file(ttt_cxx)
        .include(&out_dir)
        .flag_if_supported("-std=c++20")
        .flag_if_supported("-ffreestanding")
        .flag_if_supported("-mgeneral-regs-only")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-unwind-tables")
        .flag_if_supported("-fno-rtti")
        .flag_if_supported("-fno-threadsafe-statics")
        .cpp_set_stdlib(None)
        .compile("translation_table_cc");
}
