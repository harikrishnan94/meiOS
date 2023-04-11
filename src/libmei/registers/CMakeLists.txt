set(REGISTER_DEFINITION_FILES config.yaml exception.yaml mmu.yaml timer.yaml uart.yaml)
set(CXX_REGISTERS_HDR_DIR ${CMAKE_CURRENT_BINARY_DIR}/registers)

generate_registers(libmei ${CMAKE_CURRENT_SOURCE_DIR} ${CXX_REGISTERS_HDR_DIR}
    SOURCES ${REGISTER_DEFINITION_FILES})
add_subdirectory(test)