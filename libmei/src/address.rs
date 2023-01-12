use macros::AddressOps;
use tock_registers::{interfaces::Readable, register_bitfields, registers::InMemoryRegister};

use crate::error::{Error, Result};

pub const VIRTUAL_ADDRESS_LEVEL_IDX_BITS: usize = 9;

/// Base trait common to both Physical and Virtual Addresses
#[const_trait]
pub trait Address: Clone + Copy + Ord + core::fmt::Display {
    fn as_raw_ptr(&self) -> usize;

    fn as_ptr<T: Sized>(&self) -> *const T;

    fn as_mut_ptr<T: Sized>(&self) -> *mut T;

    fn align_offset(&self, align: usize) -> usize {
        let ptr = self.as_raw_ptr();
        ((ptr + (align - 1)) & !(align - 1)) - ptr
    }

    fn is_aligned(&self, align: usize) -> bool {
        self.align_offset(align) == 0
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddressTranslationLevel {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
}

impl VirtualAddress {
    pub fn new(val: usize) -> Result<Self> {
        let va = Self(val);

        match va.identify_ttbr_select() {
            Some(_) => Ok(va),
            None => Err(Error::InvalidVirtualAddress(val)),
        }
    }

    pub fn get_ttbr_select(&self) -> TTBR {
        self.identify_ttbr_select().unwrap()
    }

    pub fn get_idx_for_level(&self, level: &AddressTranslationLevel) -> usize {
        let reg = InMemoryRegister::<usize, VA::Register>::new(self.0);

        match level {
            AddressTranslationLevel::Zero => reg.read(VA::Level_0),
            AddressTranslationLevel::One => reg.read(VA::Level_1),
            AddressTranslationLevel::Two => reg.read(VA::Level_2),
            AddressTranslationLevel::Three => reg.read(VA::Level_3),
        }
    }

    #[allow(non_snake_case)]
    pub fn get_page_offset_4KiB(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::PageOffset_4KiB)
    }

    #[allow(non_snake_case)]
    pub fn get_page_offset_2MiB(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::PageOffset_2MiB)
    }

    #[allow(non_snake_case)]
    pub fn get_page_offset_1GiB(&self) -> usize {
        InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::PageOffset_1GiB)
    }

    fn identify_ttbr_select(&self) -> Option<TTBR> {
        match InMemoryRegister::<usize, VA::Register>::new(self.0).read(VA::TTBR_Select) {
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
        // For 4 KiB Page
        PageOffset_4KiB OFFSET(0) NUMBITS(12) [],
        // For 2 MiB Page
        PageOffset_2MiB OFFSET(0) NUMBITS(21) [],
        // For 1 GiB Page
        PageOffset_1GiB OFFSET(0) NUMBITS(30) [],

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
