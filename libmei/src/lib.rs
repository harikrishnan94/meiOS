#![cfg_attr(feature = "no_std", no_std)]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(const_trait_impl)]
#![feature(allocator_api)]
#![feature(strict_provenance)]
#![feature(slice_ptr_get)]
#![cfg_attr(test, feature(nonnull_slice_from_raw_parts))]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;

#[cfg(feature = "no_std")]
pub mod arch;

pub mod address;
pub mod address_map;
pub mod bug;
pub mod error;
pub mod mimo;
pub mod mmu;
pub mod static_bump_alloc;
pub mod vm;
