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
    text    PT_LOAD FLAGS(5);
    rodata  PT_LOAD FLAGS(4);
    data    PT_LOAD FLAGS(6);
    bss     PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = binary_load_addr;

    /* Used to identify the size of kernel image */
    __kernel_start_marker = .;

    /* Mark the start of kernel stack (grows downwards) */
    __kernel_stack_start_marker = .;

    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
    } :text

    .init :
    {
        KEEP(*(.init))
        KEEP(*(.fini))
    }

    /* Include the list of initialization functions sorted. */
    .init.array :
    {
        __init_array_start = .;
        *(.init_array)
        *(.init_array.*)
        __init_array_end = .;
    }
    __text_end = .;
    . = ALIGN(page_size); /* align to page size */

    __rodata_start = .;
    .rodata :
    {
        *(.rodata)
    } :rodata
    __rodata_end = .;
    . = ALIGN(page_size); /* align to page size */

    __data_start = .;
    .data :
    {
        *(.data)
    } :data
    __data_end = .;
    . = ALIGN(page_size); /* align to page size */

    __bss_start = .;
    .bss :
    {
        bss = .;
        *(.bss)
    } :bss
    __bss_end = .;
    __bss_size = __bss_end - __bss_start;
    . = ALIGN(page_size); /* align to page size */

    /* Used to identify the size of kernel image */
    __kernel_end_marker = .;
}