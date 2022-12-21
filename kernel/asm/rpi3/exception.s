.section .text

.global vector_table
.balign 0x800 /* 2048 */
vector_table:

/* Switch from current EL to same EL, Using SP0 */
.balign 0x80 /* 128 */
current_el_sp0_sync_handler:
    b current_el_sp0_sync

.balign 0x80 /* 128 */
current_el_sp0_irq_handler:
    b current_el_sp0_irq

.balign 0x80 /* 128 */
current_el_sp0_fiq_handler:
    b current_el_sp0_fiq

.balign 0x80 /* 128 */
current_el_sp0_serror_handler:
    b current_el_sp0_serror


/* Switch from current EL to same EL, Using SPN */
.balign 0x80 /* 128 */
current_el_spn_sync_handler:
    b current_el_spn_sync

.balign 0x80 /* 128 */
current_el_spn_irq_handler:
    b current_el_spn_irq

.balign 0x80 /* 128 */
current_el_spn_fiq_handler:
    b current_el_spn_fiq

.balign 0x80 /* 128 */
current_el_spn_serror_handler:
    b current_el_spn_serror


/* Switch from lower EL to higher EL, in aarch64 */
.balign 0x80 /* 128 */
lower_el_aarch64_sync_handler:
    b lower_el_aarch64_sync

.balign 0x80 /* 128 */
lower_el_aarch64_irq_handler:
    b lower_el_aarch64_irq

.balign 0x80 /* 128 */
lower_el_aarch64_fiq_handler:
    b lower_el_aarch64_fiq

.balign 0x80 /* 128 */
lower_el_aarch64_serror_handler:
    b lower_el_aarch64_serror


/* Switch from lower EL to higher EL, in aarch32 */
.balign 0x80 /* 128 */
lower_el_aarch32_sync_handler:
    b lower_el_aarch32_sync

.balign 0x80 /* 128 */
lower_el_aarch32_irq_handler:
    b lower_el_aarch32_irq

.balign 0x80 /* 128 */
lower_el_aarch32_fiq_handler:
    b lower_el_aarch32_fiq

.balign 0x80 /* 128 */
lower_el_aarch32_serror_handler:
    b lower_el_aarch32_serror
