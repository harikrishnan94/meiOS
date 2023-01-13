add_custom_target(linker-ld DEPENDS linker.ld)

add_executable(mei kernel.cpp)
target_include_directories(mei PRIVATE ${MEI_INCLUDE_DIR})
target_link_libraries(mei PRIVATE libmei)
target_link_options(mei PRIVATE -T ${CMAKE_CURRENT_SOURCE_DIR}/linker.ld)
set_property(TARGET mei PROPERTY LINK_DEPENDS linker-ld)

add_custom_command(TARGET mei
    POST_BUILD
    COMMAND ${CMAKE_OBJCOPY} -O binary $<TARGET_FILE:mei> kernel8.img
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    COMMENT "Creating kernel image...")
