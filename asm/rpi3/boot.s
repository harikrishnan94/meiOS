/* To keep this in the first portion of the binary. */
.section ".text.boot"

.globl _start
_start:
    /* Load current running core ID into x0 */
    mrs x0, MPIDR_EL1
    and x0, x0, {CPUID_MASK}
    /* Exit if we're not in 0th core */
    cmp x0, {BOOT_CORE_ID}
    bne 1f

    /* Get current exception level in x0 */
    mrs x0, CurrentEL
    lsr x0, x0, {EL_BITS_OFFSET}
    /* Exit if current exception level is not 2 (Hypervisor Level) */
    cmp x0, {HYP_MODE_EL}

    /* We're in 0th core and in EL 2. Start exectuting the kernel */
    beq 2f

1:
    wfe
    b 1b

2:
    /* initialize BSS */
    ldr x0, =__bss_start
    ldr x1, =__bss_end
    cmp x0, x1
    bge 4f
3:
    str xzr, [x0]
    add x0, x0, #8
    cmp x0, x1
    bne 3b

4:
    /* setup stack pointer to enable C functions */
    ldr x0, =_start
    mov sp, x0

    /* Jump to Rust code. x0 holds the function argument provided to _start_rust(). */
    b _start_rust
