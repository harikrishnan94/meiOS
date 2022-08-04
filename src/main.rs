#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(mei::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use mei::{
    exit::{exit, ExitCode},
    println,
};
mod kmain;

#[no_mangle]
pub extern "C" fn mei_main() -> ExitCode {
    kmain::kmain()
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

#[panic_handler]
fn on_panic(_info: &PanicInfo) -> ! {
    println!("{}", _info);
    exit(ExitCode::Failure)
}
