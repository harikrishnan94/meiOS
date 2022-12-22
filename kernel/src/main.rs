#![no_std]
#![no_main]
#![feature(asm_const)]

use aarch64_cpu::{asm, registers::*};
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
    }

    loop {
        asm::wfe();
    }
}
