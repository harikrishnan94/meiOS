#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]

use aarch64_cpu::{asm, registers::*};
use libmei::{
    boot::{switch_from_el1_to_el0, switch_from_el2_to_el1},
    exception, print, println, timer, uart,
};
use tock_registers::interfaces::Readable;

fn mei_main() -> ! {
    print!("\nWelcome to meiOS... ");
    println!("We're at Exception Level {}", CurrentEL.read(CurrentEL::EL));

    unsafe {
        timer::enable();
        uart::enable();
        exception::handler_init();
        exception::enable_irq();
        drop_to_el0()
    }
}

const EL0_STACK_SIZE: usize = 8192;
static EL0_STACK: [u8; EL0_STACK_SIZE] = [0; EL0_STACK_SIZE];

/// Entry point for EL0 (user space)
#[naked]
unsafe extern "C" fn el0_main() -> ! {
    // Infinite Loop
    core::arch::asm!("1: b 1b", options(noreturn));
}

#[no_mangle]
unsafe extern "C" fn _start_rust(phy_stack_ptr: u64) -> ! {
    switch_from_el2_to_el1(phy_stack_ptr, mei_main as *const ());

    // Use `eret` to "return" to EL1. This results in execution of mei_main() in EL1.
    asm::eret()
}

unsafe extern "C" fn drop_to_el0() -> ! {
    switch_from_el1_to_el0(EL0_STACK.as_ptr() as u64, el0_main as *const ());

    // Use `eret` to "return" to EL0. This results in execution of el0_main() in EL0.
    asm::eret()
}
