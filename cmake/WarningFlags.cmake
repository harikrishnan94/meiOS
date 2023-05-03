option(ENABLE_WERROR "Enable Werror." ON)

function(add_warning_flags)
    add_compile_options(-Wall -Wextra -Wno-unused-function)

    if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
        add_compile_options(-Wunused-const-variable)
    endif()

    if(ENABLE_WERROR)
        add_compile_options(-Werror)
    endif()
endfunction()
