option(MEI_RASPI_BOARD
    "RaspberryPi Board to compile mei kernel for. Supported options RaspberryPi 3 (raspi3) and RaspberryPi 4 (raspi4)"
    "raspi3")

if(MEI_RASPI_BOARD STREQUAL "raspi3")
    add_library(boot STATIC rpi3.S)
elseif(MEI_RASPI_BOARD STREQUAL "raspi4")
    add_library(boot STATIC rpi3.S)
else()
    message(FATAL_ERROR "Unsupported RaspberryPi board: ${MEI_RASPI_BOARD}")
endif()

target_include_directories(boot PUBLIC ${MEI_INCLUDE_DIR})
target_link_libraries(boot PUBLIC -static-libgcc -static -lgcc -lc -lstdc++)