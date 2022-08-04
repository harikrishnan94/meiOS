#![no_std]
#![cfg_attr(test, no_main)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::arch::global_asm;
use core::panic::PanicInfo;
use exit::{exit, ExitCode};

pub mod arch_regs;
pub mod board_id;
pub mod exit;
pub mod mimo;
pub mod uart;

global_asm!(include_str!("../boot/rpi3.S"));

pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    exit(ExitCode::Failure)
}

/// Entry point for `cargo xtest`

#[no_mangle]
#[cfg(test)]
pub extern "C" fn mei_main() -> ExitCode {
    test_main();
    ExitCode::Success
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
