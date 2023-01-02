#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]

use aarch64_cpu::registers::*;
use libmei::{exception, print, println, timer, uart};
use tock_registers::interfaces::Readable;

mod boot;

fn mei_main() -> ! {
    print!("\nWelcome to meiOS... ");
    println!("We're at Exception Level {}", CurrentEL.read(CurrentEL::EL));

    unsafe {
        timer::enable();
        uart::enable();
        exception::handler_init();
        exception::enable_irq();
        boot::drop_to_el0()
    }
}

/// .
///
/// # Safety
///
/// Entry point for EL0 (user space)
#[no_mangle]
#[naked]
unsafe extern "C" fn el0_main() -> ! {
    // Infinite Loop
    core::arch::asm!("1: b 1b", options(noreturn));
}
