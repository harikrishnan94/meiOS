#![cfg_attr(feature = "no_std", no_std)]
#![feature(
    naked_functions,
    asm_const,
    const_trait_impl,
    allocator_api,
    strict_provenance,
    slice_ptr_get
)]
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
