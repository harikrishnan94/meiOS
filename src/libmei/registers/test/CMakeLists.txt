if(MEI_ENABLE_CORE_TESTS)
    define_simple_test(registers_test)

    set(REGISTER_DEFINITION_FILES test_regs.yaml)
    set(CXX_REGISTERS_HDR_DIR ${CMAKE_CURRENT_BINARY_DIR}/registers)
    generate_registers(registers_test ${CMAKE_CURRENT_SOURCE_DIR} ${CMAKE_CURRENT_BINARY_DIR}
        SOURCES test_regs.yaml)
    target_link_libraries(registers_test PRIVATE libmei)
endif()
