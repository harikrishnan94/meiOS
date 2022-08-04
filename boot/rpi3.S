// Refer https://wiki.osdev.org/Raspberry_Pi_Bare_Bones for more details

// AArch64 mode

// To keep this in the first portion of the binary.
.section ".text.boot"

// Make _start global
.globl _start
// In 64 bit mode, bootcode is loaded at 0x80000 not in 0x8000.
    .org 0x80000

// Entry point for kernel.
// Registers:
// x0 -> 32bit pointer to DTB in memory (primary core only) / 0 (secondary cores)
// x1 -> 0
// x2 -> 0
// x3 -> 0
// x4 -> 32 bit kernel entry point, _start location.
_start:
    mrs x1, mpidr_el1
    and x1, x1, #3
    cbz x1, 2f
    // We're not on the main core, so hang in an infinite wait loop
1:  wfe
    b 1b

2:  // We're on the main core!
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

    // Jump into rust code
4:  bl mei_main
    bl _exit

// Exit to qemu
// x0 <- Return code
.globl _exit
_exit:
    /* 0x20026 == ADP_Stopped_ApplicationExit */
    mov x1, #0x26
    movk x1, #2, lsl #16
    str x1, [sp,#0]

    /* Exit status code. Host QEMU process exits with that status. */
    /* x0 must contain the exit code */
    str x0, [sp,#8]

    /* x1 contains the address of parameter block.
     * Any memory address could be used. */
    mov x1, sp

    /* SYS_EXIT */
    mov w0, #0x18

    /* Do the semihosting call on A64. */
    hlt 0xf000
