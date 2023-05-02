option(ENABLE_WERROR "Enable Werror." ON)

function(add_warning_flags)
    add_compile_options(-Wall -Wextra -Wno-unused-function -Wunused-const-variable)

    if(ENABLE_WERROR)
        add_compile_options(-Werror)
    endif()
endfunction()
