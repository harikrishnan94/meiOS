#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(mei::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use mei::exit::ExitCode;
use mei::println;
use mei::test_panic_handler;

#[no_mangle]
pub extern "C" fn mei_main() -> ExitCode {
    test_main();
    ExitCode::Success
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
fn test_println() {
    println!("test_println output");
}
