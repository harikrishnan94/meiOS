#![no_std]
#![no_main]

use core::panic::PanicInfo;
use mei::{exit, exit::ExitCode};
use mei::{print, println};

#[no_mangle]
pub extern "C" fn mei_main() -> ! {
    should_fail();
    unimplemented!()
}

fn should_fail() {
    print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[cfg(test)]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[ok]");
    exit::exit(ExitCode::Success)
}
