#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

use core::arch::global_asm;

use crate::board_id::{get_raspi_board, RaspiBoard};
mod arch_regs;
mod board_id;
mod mimo;
mod panic;
mod uart;

global_asm!(include_str!("../boot/rpi3.S"));

#[no_mangle]
pub extern "C" fn mei_main() -> i32 {
    match get_raspi_board() {
        RaspiBoard::ONE => println!("Welcome to Raspberry PI 1"),
        RaspiBoard::TWO => println!("Welcome to Raspberry PI 2"),
        RaspiBoard::THREE => println!("Welcome to Raspberry PI 3"),
        RaspiBoard::FOUR => println!("Welcome to Raspberry PI 4"),
        _ => println!("Welcome to Raspberry PI XXX"),
    }

    0
}
