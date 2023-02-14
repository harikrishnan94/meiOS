#![cfg_attr(feature = "no_std", no_std)]
#![feature(
    naked_functions,
    asm_const,
    const_trait_impl,
    int_roundings,
    alloc_layout_extra,
    allocator_api,
    strict_provenance,
    slice_ptr_get,
    nonnull_slice_from_raw_parts
)]

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
pub mod vm;
