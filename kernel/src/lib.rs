#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#[macro_use]
extern crate lazy_static;

pub mod exception;
pub mod gic;
pub mod gpio;
pub mod panic;
pub mod static_bump_alloc;
pub mod timer;
pub mod uart;
