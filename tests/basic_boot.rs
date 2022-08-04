#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(mei::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use mei::exit::ExitCode;
use mei::println;

#[no_mangle]
pub extern "C" fn mei_main() -> ExitCode {
    test_main();
    ExitCode::Success
}

#[test_case]
fn test_println() {
    println!("test_println output");
}
