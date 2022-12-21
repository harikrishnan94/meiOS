#![no_std]
#![no_main]
#![feature(asm_const)]

use aarch64_cpu::registers::*;
use libmei::{panic::exit_success, print, println};
use tock_registers::interfaces::Readable;

mod boot;

fn mei_main() -> ! {
    print!("\nWelcome to meiOS... ");
    println!("We're at Exception Level {}", CurrentEL.read(CurrentEL::EL));
    exit_success()
}
