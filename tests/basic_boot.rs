#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rutos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use rutos::{inifinite_loop, println};

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    test_main();

    inifinite_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rutos::test_panic_handler(info)
}

#[test_case]
fn test_println() {
    println!("test_println output");
}
