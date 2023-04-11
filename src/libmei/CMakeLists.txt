include(FetchContent)

FetchContent_Declare(
    ktl
    GIT_REPOSITORY https://github.com/harikrishnan94/ktl
    GIT_TAG "0.0.3"
)
FetchContent_MakeAvailable(ktl)

add_library(libmei STATIC dummy.cpp)
target_include_directories(libmei PUBLIC ${MEI_INCLUDE_DIR})
target_link_libraries(libmei PUBLIC ktl)

add_subdirectory(registers)