add_subdirectory(boot)

add_library(arch STATIC exception.S)
target_link_libraries(arch PUBLIC boot)