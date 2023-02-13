use crate::address::AddressTranslationLevel;

pub mod consts {
    use crate::address;

    /// General consts
    pub const ONE_GIB: usize = 1024 * 1024 * 1024;
    pub const TWO_MIB: usize = 2 * 1024 * 1024;
    pub const FOUR_KIB: usize = 4 * 1024;

    /// Translation Consts

    pub const VIRTUAL_ADDRESS_IGNORE_MSB: u32 = address::VIRTUAL_ADDRESS_IGNORE_MSB;
    pub const VIRTUAL_ADDRESS_LEVEL_IDX_BITS: u32 = address::VIRTUAL_ADDRESS_LEVEL_IDX_BITS;

    pub const VIRTUAL_ADDRESS_NBITS: u32 = u64::BITS - VIRTUAL_ADDRESS_IGNORE_MSB;
    pub const VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS: u32 = FOUR_KIB.ilog2();
    pub const MAX_TRANSLATION_LEVELS: usize = ((VIRTUAL_ADDRESS_NBITS
        - VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS)
        / VIRTUAL_ADDRESS_LEVEL_IDX_BITS) as usize;
}

pub const fn get_vaddr_spacing_per_entry(level: &AddressTranslationLevel) -> usize {
    match level {
        AddressTranslationLevel::Zero => 512 * consts::ONE_GIB,
        AddressTranslationLevel::One => consts::ONE_GIB,
        AddressTranslationLevel::Two => consts::TWO_MIB,
        AddressTranslationLevel::Three => consts::FOUR_KIB,
    }
}
