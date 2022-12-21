use aarch64_cpu::{asm, registers::*};
use core::arch::global_asm;
use tock_registers::interfaces::Writeable;

global_asm!(
    include_str!("../asm/rpi3/boot.s"),
    CPUID_MASK = const ((1 << 2) - 1), /* MPIDR_EL1's last 2 bits contain the current cpu */
    BOOT_CORE_ID = const 0,
    EL_BITS_OFFSET = const 2, /* CurrentEL's 2:3 contains the exception level */
    HYP_MODE_EL = const 2 /* Hypervisor mode EL is 2 */
);

/// Prepares the transition from EL2 to EL1.
///
/// # Safety
///
/// - The HW state of EL1 must be prepared in a sound way.
unsafe fn prepare_switch_from_el2_to_el1(phy_stack_ptr: u64) {
    // Set EL1 execution state to AArch64.
    HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);

    // Setup for fake exception return

    // First, fake a saved program status where all interrupts were masked and SP_EL1 was used as a
    // stack pointer.
    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h,
    );

    // Second, let the link register point to mei_main().
    ELR_EL2.set(crate::mei_main as *const () as u64);

    // Set up SP_EL1 (stack pointer), which will be used by EL1 once we "return" to it. Since there
    // are no plans to ever return to EL2, just re-use the same stack.
    SP_EL1.set(phy_stack_ptr);
}

#[no_mangle]
pub(crate) unsafe extern "C" fn _start_rust(phy_stack_ptr: u64) -> ! {
    prepare_switch_from_el2_to_el1(phy_stack_ptr);

    // Use `eret` to "return" to EL1. This results in execution of mei_main() in EL1.
    asm::eret()
}
