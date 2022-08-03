// Refer https://wiki.osdev.org/Raspberry_Pi_Bare_Bones for more details

#include "mei/kmain.h"

// AArch64 mode

// In 64 bit mode, bootcode is loaded at 0x80000 not in 0x8000.
#define BOOT_ADDRESS 0x80000


// To keep this in the first portion of the binary.
.section ".text.boot"

// Make _start global
.globl _start
    .org BOOT_ADDRESS

// Entry point for kernel.
// Registers:
// x0 -> 32bit pointer to DTB in memory (primary core only) / 0 (secondary cores)
// x1 -> 0
// x2 -> 0
// x3 -> 0
// x4 -> 32 bit kernel entry point, _start location.
_start:
    // set the stack before our code.
    ldr x5, =_start
    mov sp, x5

    // clear bss
    ldr x5, =__bss_start
    ldr w6, =__bss_size
3:  cbz w6, 4f
    str xzr, [x5], #8
    sub w6, w6, #1
    cbnz w6, 3b

    // Jump into C code. Shouldn't return
4:  bl MEI_MAIN
    // for failsafe, halt this core too
5:  wfe
	b 5b

