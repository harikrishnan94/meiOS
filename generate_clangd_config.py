#!/usr/bin/env python3

import sys

# Path to compiler installation directory. Must contain 'bin', 'lib', 'include', directories
gcc_dir = sys.argv[1]
gcc_ver = sys.argv[2]  # Exact compiler version, ex: 12.2.1

clangd_config = """CompileFlags:
  Add: ["--sysroot={dir}/lib/gcc/aarch64-none-elf/{ver}/", \
"-I{dir}/aarch64-none-elf/include", \
"-I{dir}/aarch64-none-elf/include/c++/{ver}", \
"-I{dir}/aarch64-none-elf/include/c++/{ver}/aarch64-none-elf"]
  Remove: [-isysroot*]
  Compiler: {dir}/bin/aarch64-none-elf-g++
  CompilationDatabase: "./out/build/debug"
"""

print(clangd_config.format(dir=gcc_dir, ver=gcc_ver))
