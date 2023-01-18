#[macro_use]
extern crate anyhow;

use anyhow::Result;
use std::{env, path::Path};
use walkdir::WalkDir;

fn main() -> Result<()> {
    let ttt_rs = "src/mmu/translation_table.rs";
    let ttt_cxx = "src/cxx/translation_table.cpp";
    let out_dir = env::var("OUT_DIR").unwrap();

    let mut cxx = cxx_build::bridge(ttt_rs);
    let is_host = target_is_host().unwrap_or(true);

    choose_compiler(&mut cxx, is_host)?;
    if is_host {
        cxx.define("USE_THREAD_LOCAL", "1");
    }

    cxx.file(ttt_cxx)
        .include(&out_dir)
        .flag("-std=c++20")
        .flag("-ffreestanding")
        .flag("-mgeneral-regs-only")
        .flag("-fno-exceptions")
        .flag("-fno-unwind-tables")
        .flag("-fno-rtti")
        .flag("-fno-threadsafe-statics")
        .warnings(true)
        .warnings_into_errors(true)
        .cpp_set_stdlib(None)
        .compile("translation_table_cc");

    Ok(())
}

fn choose_compiler(cxx: &mut cc::Build, is_host: bool) -> Result<()> {
    const CPP_COMPILER_NAME: &str = "aarch64-none-elf-g++";
    if is_host {
        return Ok(());
    }

    let gcc_install_dir_var = env::var("AARCH64_NONE_ELF_TOOLCHAIN")?;
    let gcc_install_dir = Path::new(&gcc_install_dir_var);
    for entry in WalkDir::new(gcc_install_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
    {
        let f_name = entry.file_name().to_string_lossy();

        if f_name == CPP_COMPILER_NAME {
            cxx.compiler(entry.path().to_str().unwrap());
            println!("cargo:warning=Using G++={}", entry.path().to_str().unwrap());
            return Ok(());
        }
    }

    bail!("Cannot Find G++ Executable");
}

fn target_is_host() -> Result<bool> {
    Ok(env::var("TARGET")? == env::var("HOST")?)
}
