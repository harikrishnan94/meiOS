add_subdirectory(arch)

add_library(libmei STATIC lib.c)
target_include_directories(libmei PUBLIC ${MEI_INCLUDE_DIR})
target_compile_definitions(libmei PUBLIC
    FMT_HEADER_ONLY
    FMT_DISABLE_FLOAT
    FMT_STATIC_THOUSANDS_SEPARATOR=','
    FMT_USE_EXTERNAL_ASSERT
    FMT_NO_DYN_ALLOC
    FMT_NO_STDIO
)
target_link_libraries(libmei PUBLIC arch)

add_subdirectory(registers)