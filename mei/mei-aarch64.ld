/* The physical address at which the the kernel binary will be loaded by the Raspberry's firmware */
binary_load_addr = 0x80000;
page_size = 4096;

ENTRY(_start)

/* Flags:
 *     4 == R
 *     5 == RX
 *     6 == RW
 *
 * Segments are marked PT_LOAD below so that the ELF file provides virtual and physical addresses.
 * It doesn't mean all of them need actually be loaded.
 */
PHDRS
{
    segment_code            PT_LOAD FLAGS(5);
    segment_data            PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = binary_load_addr;

    /* Used to identify the size of kernel image */
    __kernel_start_marker = .;

    /* Mark the start of kernel stack (grows downwards) */
    __kernel_stack_start_marker = .;

    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
    }
    . = ALIGN(page_size);

    .rodata :
    {
        *(.rodata)
    }
    . = ALIGN(page_size);

    .data :
    {
        *(.data)
    }
    . = ALIGN(page_size);

    .bss :
    {
        __bss_start = .;
        *(.bss)
        __bss_end = .;
    }
    . = ALIGN(page_size);

    /* Used to identify the size of kernel image */
    __kernel_end_marker = .;
}