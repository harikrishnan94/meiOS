use macros::AddressOps;
use tock_registers::{interfaces::Readable, register_bitfields, registers::InMemoryRegister};

use crate::error::Error;

/// Base trait common to both Physical and Virtual Addresses
#[const_trait]
pub trait Address: Clone + Copy + Ord + core::fmt::Display {
    fn as_raw_ptr(&self) -> usize;

    fn as_ptr<T: Sized>(&self) -> *const T;

    fn as_mut_ptr<T: Sized>(&self) -> *mut T;
}

/// Physical Address
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, AddressOps)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }
}

impl core::fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:X}_P", self.0)
    }
}

/// Virtual Address
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, AddressOps)]
pub struct VirtualAddress(usize);

pub enum TTBR {
    Zero = 0,
    One = 1,
}

impl VirtualAddress {
    pub fn new(val: usize) -> Result<Self, Error> {
        let va = Self(val);

        if va.identify_ttbr_select().is_some() {
            Ok(va)
        } else {
            Err(Error::InvalidVirtualAddress(val))
        }
    }

    pub fn get_ttbr_select(&self) -> TTBR {
        self.identify_ttbr_select().unwrap()
    }

    pub fn get_level3_ind(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::Level_3)
    }

    pub fn get_level2_ind(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::Level_2)
    }

    pub fn get_level1_ind(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::Level_1)
    }

    pub fn get_level0_ind(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::Level_0)
    }

    pub fn get_page_offset(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::PageOffset)
    }

    fn identify_ttbr_select(&self) -> Option<TTBR> {
        match InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::PageOffset) {
            0xFFFF => Some(TTBR::One),
            0x0000 => Some(TTBR::Zero),
            _ => None,
        }
    }
}

impl core::fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:X}_V", self.0)
    }
}

// Virtual Address with 4KB granule and 4 level translation
register_bitfields![usize,
    VA [
        /// Offset within page
        PageOffset OFFSET(0) NUMBITS(12) [],

        /// Level 3 Index
        Level_3 OFFSET(12) NUMBITS(9) [],

        /// Level 2 Index
        Level_2 OFFSET(21) NUMBITS(9) [],

        /// Level 1 Index
        Level_1 OFFSET(30) NUMBITS(9) [],

        /// Level 0 Index
        Level_0 OFFSET(39) NUMBITS(9) [],

        /// TTBR select
        TTBR_Select OFFSET(48) NUMBITS(16) [],
    ]
];
