function(generate_registers target src_directory output_directory)
    cmake_parse_arguments(
        PARSE_ARGV 2 generate_registers "" ""
        "SOURCES")

    set(gen_reg_def_py "${PROJECT_SOURCE_DIR}/generate_register_definitions.py")

    file(MAKE_DIRECTORY ${output_directory})
    add_custom_target(${target}-regs DEPENDS ${gen_reg_def_py} ${REGISTER_DEFINITION_FILES})
    add_custom_command(TARGET ${target}-regs
        PRE_BUILD
        COMMAND ${gen_reg_def_py} ${src_directory} ${generate_registers_SOURCES}
        WORKING_DIRECTORY ${output_directory}
        COMMENT "Generating registers file...")
    add_dependencies(${target} ${target}-regs)
    target_include_directories(${target} PUBLIC ${output_directory})
endfunction()
