include(FetchContent)

FetchContent_Declare(
    etl
    GIT_REPOSITORY https://github.com/ETLCPP/etl
    GIT_TAG "20.35.10"
)

FetchContent_MakeAvailable(etl)

add_subdirectory(arch)

add_library(libmei STATIC uart.cpp print.cpp)
target_include_directories(libmei PUBLIC ${MEI_INCLUDE_DIR})
target_compile_definitions(libmei PUBLIC

    # FMT related config
    FMT_HEADER_ONLY
    FMT_DISABLE_FLOAT
    FMT_STATIC_THOUSANDS_SEPARATOR
    FMT_USE_EXTERNAL_ASSERT
    FMT_NO_DYN_ALLOC
    FMT_NO_STDIO
)
target_link_libraries(libmei PUBLIC arch PUBLIC etl::etl)

add_subdirectory(registers)