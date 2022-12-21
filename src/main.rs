#![no_std]
#![no_main]
#[macro_use]
extern crate lazy_static;

use core::arch::global_asm;
mod exception;
mod gpio;
mod panic;
mod uart;

global_asm!(include_str!("../asm/rpi3/boot.s"));
global_asm!(include_str!("../asm/rpi3/vector_table.s"));

#[no_mangle]
pub(crate) extern "C" fn mei_main() {
    println!("Welcome to meiOS");
}

#[no_mangle]
pub(crate) extern "C" fn init_bss(bss_start: *mut u8, bss_end: *mut u8) {
    unsafe {
        let size = bss_end.offset_from(bss_start);
        if size > 0 {
            core::intrinsics::write_bytes(bss_start, 0, size as usize);
        }
    }
}
