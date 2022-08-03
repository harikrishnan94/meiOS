add_library(boot OBJECT rpi3.S)
target_include_directories(boot PUBLIC ${MEI_INCLUDE_DIR})