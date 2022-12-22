#![no_std]
#![no_main]
#![feature(asm_const)]

use aarch64_cpu::{asm, registers::*};
use libmei::{
    exception::{enable_irq, handler_init},
    print, println,
    timer::init_timer,
};
use tock_registers::interfaces::Readable;

mod boot;

fn mei_main() -> ! {
    print!("\nWelcome to meiOS... ");
    println!("We're at Exception Level {}", CurrentEL.read(CurrentEL::EL));

    unsafe {
        init_timer();
        handler_init();
        enable_irq();
    }

    loop {
        asm::wfe();
    }
}
