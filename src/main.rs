#![no_std]
#![feature(asm_const)]
#![no_main]
#[macro_use]
extern crate lazy_static;

use aarch64_cpu::registers::*;
use tock_registers::interfaces::Readable;

mod boot;
mod exception;
mod gpio;
mod panic;
mod uart;
use qemu_exit::QEMUExit;

fn mei_main() -> ! {
    print!("\nWelcome to meiOS... ");
    println!("We're at Exception Level {}", CurrentEL.read(CurrentEL::EL));
    QEMU_EXIT_HANDLE.exit_success()
}

#[cfg(target_arch = "aarch64")]
static QEMU_EXIT_HANDLE: qemu_exit::aarch64::AArch64 = qemu_exit::AArch64::new();

#[no_mangle]
pub(crate) extern "C" fn exit(exit_code: u32) -> ! {
    QEMU_EXIT_HANDLE.exit(exit_code);
}
