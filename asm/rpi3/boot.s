/* To keep this in the first portion of the binary. */
.section ".text.boot"

.globl _start
_start:
    /* Load current running core ID into x0 */
    mrs x0, mpidr_el1
    and x0, x0, #3
    /* Exit if we're not in 0th core */
    cmp x0, #0

    /* We're in 0th core. Start exectuting the kernel */
    beq 2f

1:
    wfe
    b 1b

2:
    /* Get current exception level in x0 */
    mrs x0, currentel
    lsr x0, x0, #2
    /* Exit if current exception level is not 2 (Hypervisor Level) */
    cmp x0, #2
    bne 1b

    /* Setup system control register */
    msr sctlr_el1, xzr
    mov x0, #(1 << 31) /* Enable AArch64 Mode */
    msr hcr_el2, x0

    /* Setup for fake exception return */
    mov x0, #0b1111000101 /* Sets: (D, A, I, F, SP_EL1, EL1) */
    msr spsr_el2, x0
    adr x0, 3f
    msr elr_el2, x0
    eret

    /* EL1 Entry (Kernel Mode) */
3:
    /* TODO: Setup MMU and enable Stack alignment check in sctlr_el1 */
    /* setup stack pointer to enable C functions */
    mov sp, #0x80000

    /* initialize BSS */
    ldr x0, =__bss_start
    ldr x1, =__bss_end
    bl init_bss

    /* Load vector_table into vbar_el1 */
    ldr x0, =vector_table
    msr vbar_el1, x0

    /* call mei kernel */
    bl mei_main
    bl exit