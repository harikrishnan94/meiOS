set(CMAKE_SYSTEM_NAME Generic)
set(CMAKE_SYSTEM_PROCESSOR ARM)

if(NOT ARM_GCC_TOOLCHAIN_PATH)
    set(ARM_GCC_TOOLCHAIN_PATH "$ENV{ARM_GCC_TOOLCHAIN_PATH}")
endif()

if(NOT ARM_CLANG_TOOLCHAIN_PATH)
    set(ARM_CLANG_TOOLCHAIN_PATH "$ENV{ARM_CLANG_TOOLCHAIN_PATH}")
endif()

set(CROSS_COMPILE aarch64-none-elf-)

option(USE_CLANG "Use Clang toolchain to build meiOS" OFF)

if(USE_CLANG)
    set(CMAKE_C_COMPILER "${ARM_CLANG_TOOLCHAIN_PATH}clang")
    set(CMAKE_CXX_COMPILER "${ARM_CLANG_TOOLCHAIN_PATH}clang++")
else()
    set(CMAKE_C_COMPILER "${ARM_GCC_TOOLCHAIN_PATH}${CROSS_COMPILE}gcc")
    set(CMAKE_CXX_COMPILER "${ARM_GCC_TOOLCHAIN_PATH}${CROSS_COMPILE}g++")
endif()

set(CMAKE_C_FLAGS_INIT "-ffreestanding -mgeneral-regs-only -fPIC")
set(CMAKE_CXX_FLAGS_INIT
    "${CMAKE_C_FLAGS_INIT} -fno-exceptions -fno-unwind-tables -fno-rtti -fno-threadsafe-statics")
set(CMAKE_EXE_LINKER_FLAGS_INIT "-nostdlib")

if(USE_CLANG)
    set(CMAKE_C_FLAGS_INIT "--target=aarch64-elf ${CMAKE_C_FLAGS_INIT}")
    set(CMAKE_CXX_FLAGS_INIT "--target=aarch64-elf ${CMAKE_CXX_FLAGS_INIT}")
endif()

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)