cmake_minimum_required(VERSION 3.23)

project(
    meiOS
    VERSION 0.0.1
    HOMEPAGE_URL "https://github.com/harikrishnan94/meiOS"
    DESCRIPTION "An Unix like kernel for Raspberry pi 3/4 written in C/C++"
    LANGUAGES C CXX ASM)

set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${CMAKE_SOURCE_DIR}/cmake/")
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
set(CMAKE_CXX_STANDARD 20)

find_program(CCACHE_PROGRAM ccache)

if(CCACHE_PROGRAM)
    set_property(GLOBAL PROPERTY RULE_LAUNCH_COMPILE "${CCACHE_PROGRAM}")
endif()

set(MEI_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/include")

include(WarningFlags)
include(GenerateRegisters)

add_warning_flags()
add_subdirectory(src)
