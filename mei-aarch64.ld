/* The physical address at which the the kernel binary will be loaded by the Raspberry's firmware */
binary_load_addr = 0x80000;

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
    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
    }
    . = ALIGN(4096); /* align to page size */

    .rodata :
    {
        *(.rodata)
    }
    . = ALIGN(4096); /* align to page size */

    .data :
    {
        *(.data)
    }
    . = ALIGN(4096); /* align to page size */

    .bss :
    {
        __bss_start = .;
        *(.bss)
        __bss_end = .;
    }
    . = ALIGN(4096); /* align to page size */
}