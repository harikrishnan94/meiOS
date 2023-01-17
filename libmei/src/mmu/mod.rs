use aarch64_cpu::{
    asm::barrier::{isb, SY},
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1},
};
use tock_registers::{
    interfaces::{ReadWriteable, Writeable},
    register_bitfields,
};

use crate::address::VIRTUAL_ADDRESS_LEVEL_IDX_BITS;

pub const GRANULE_SIZE: usize = 4096;
pub const GRANULE_SIZE_BITS: u32 = GRANULE_SIZE.ilog2();

pub const TRANSLATION_TABLE_DESC_ALIGN: usize =
    core::mem::size_of::<u64>() * (1 << VIRTUAL_ADDRESS_LEVEL_IDX_BITS);
pub const TRANSLATION_TABLE_DESC_ALIGN_BITS: u32 = TRANSLATION_TABLE_DESC_ALIGN.ilog2();

pub const OUTPUT_ADDR_BITS: u32 = 48;
pub const NEXT_LEVEL_TABLE_ADDR_BITS: u32 = 36;
pub const NEXT_LEVEL_TABLE_ADDR_SHIFT: u32 = OUTPUT_ADDR_BITS - NEXT_LEVEL_TABLE_ADDR_BITS;
pub const LEVEL_1_OUTPUT_ADDR_BITS: u32 = 18;
pub const LEVEL_1_OUTPUT_ADDR_SHIFT: u32 = OUTPUT_ADDR_BITS - LEVEL_1_OUTPUT_ADDR_BITS;
pub const LEVEL_2_OUTPUT_ADDR_BITS: u32 = 27;
pub const LEVEL_2_OUTPUT_ADDR_SHIFT: u32 = OUTPUT_ADDR_BITS - LEVEL_2_OUTPUT_ADDR_BITS;
pub const LEVEL_3_OUTPUT_ADDR_BITS: u32 = 36;
pub const LEVEL_3_OUTPUT_ADDR_SHIFT: u32 = OUTPUT_ADDR_BITS - LEVEL_3_OUTPUT_ADDR_BITS;

mod translation_table;

/// Setup all registers before enabling MMU
/// Also return the value to be written to SCTLR_EL1 for enabling MMU.
pub fn setup_mmu() {
    setup_ttbr1_entries();
    setup_ttbr0_entries();
    config_4kb_48bit_virtual_address_space();
    config_el1_memory_attributes();
    enable_mmu();
}

/// Setup Virtual Memory for Kernel Space (TTBR1)
fn setup_ttbr1_entries() {
    todo!()
}

/// Setup Virtual Memory for User Space (TTBR0)
fn setup_ttbr0_entries() {
    todo!()
}

/// Setup VA space for both Kernel and User space to contain 48 bits and 4KB granule
/// This means there are 4 levels of Translation required to obtain Physical address
/// from Virtual address
fn config_4kb_48bit_virtual_address_space() {
    TCR_EL1.write(
        TCR_EL1::A1::TTBR0
            + TCR_EL1::IPS::Bits_48
            + TCR_EL1::TG0::KiB_4
            + TCR_EL1::TG1::KiB_4
            + TCR_EL1::SH1::Inner
            + TCR_EL1::SH0::Inner
            + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::T0SZ.val(16) // 16 MSB's are ignored
            + TCR_EL1::T1SZ.val(16), // 16 MSB's are ignored
    );

    isb(SY);
}

/// Setup Memory Attribute Indirection Register to include Normal and Device Memory
fn config_el1_memory_attributes() {
    // Define the memory types being mapped.
    MAIR_EL1.write(
        // Attribute 1 - Cacheable normal DRAM.
        MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
    MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc +

    // Attribute 0 - Device.
    MAIR_EL1::Attr0_Device::nonGathering_nonReordering_EarlyWriteAck,
    );
}

/// Enable MMU by configuring SCTLR_EL1
fn enable_mmu() {
    SCTLR_EL1.modify(
        SCTLR_EL1::M::Enable
            + SCTLR_EL1::WXN::Enable
            + SCTLR_EL1::SA0::Enable
            + SCTLR_EL1::SA::Enable
            + SCTLR_EL1::I::Cacheable
            + SCTLR_EL1::C::Cacheable,
    );

    isb(SY);
}

register_bitfields! {u64,
    // A table descriptor (level 0), as per ARMv8-A Architecture Reference Manual Figure D8-12.
    STAGE1_TABLE_DESCRIPTOR [
        /// Physical address of the next descriptor.
        NEXT_LEVEL_TABLE_ADDR OFFSET(12) NUMBITS(36) [], // [47:12]

        TYPE OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ],

    // Block descriptors for level 1 and 2, as per ARMv8-A Architecture Reference Manual Figure D8-14.
    // Block descriptors can be used to implement Hugepages by compressing Translation Table.
    STAGE1_BLOCK_DESCRIPTOR [
        /// Bits for Software Use
        SWUSE OFFSET(55) NUMBITS(4) [],

        /// Unprivileged execute-never.
        UXN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Bits [47:30] of Output Address. Points to a 1GiB Physical Page.
        OUTPUT_ADDR_1GiB OFFSET(30) NUMBITS(18) [], // [47:30]
        /// Bits [47:21] of Output Address. Points to a 2MiB Physical Page.
        OUTPUT_ADDR_2MiB OFFSET(21) NUMBITS(27) [], // [47:21]

        /// Access flag.
        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shareability field.
        SH OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Access Permissions.
        AP OFFSET(6) NUMBITS(2) [
            RW_EL1 = 0b00,
            RW_EL1_EL0 = 0b01,
            RO_EL1 = 0b10,
            RO_EL1_EL0 = 0b11
        ],

        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],

        TYPE OFFSET(1) NUMBITS(1) [
            Reserved_Invalid = 1,
            Block = 0
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ],

    // A level 3 page descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-17.
    STAGE1_PAGE_DESCRIPTOR [
        /// Bits for Software Use
        SWUSE OFFSET(55) NUMBITS(4) [],

        /// Unprivileged execute-never.
        UXN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Bits [47:12] of Output Address. Points to a 4KiB Physical Page.
        OUTPUT_ADDR_4KiB OFFSET(12) NUMBITS(36) [], // [47:12]

        /// Access flag.
        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shareability field.
        SH OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Access Permissions.
        AP OFFSET(6) NUMBITS(2) [
            RW_EL1 = 0b00,
            RW_EL1_EL0 = 0b01,
            RO_EL1 = 0b10,
            RO_EL1_EL0 = 0b11
        ],

        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],

        TYPE OFFSET(1) NUMBITS(1) [
            Reserved_Invalid = 0,
            Page = 1
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ],

    // A level 1/2 block or level 3 page descriptor.
    STAGE1_LAST_LEVEL_DESCRIPTOR [
        /// Bits for Software Use
        SWUSE OFFSET(55) NUMBITS(4) [],

        /// Unprivileged execute-never.
        UXN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Bits [47:12] of Output Address. Points to a 4KiB Physical Page. (Level 3)
        OUTPUT_ADDR_4KiB OFFSET(12) NUMBITS(36) [], // [47:12]
        /// Bits [47:21] of Output Address. Points to a 2MiB Physical Page. (Level 2)
        OUTPUT_ADDR_2MiB OFFSET(21) NUMBITS(27) [], // [47:21]
        /// Bits [47:30] of Output Address. Points to a 1GiB Physical Page. (Level 1)
        OUTPUT_ADDR_1GiB OFFSET(30) NUMBITS(18) [], // [47:30]

        /// Access flag.
        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shareability field.
        SH OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Access Permissions.
        AP OFFSET(6) NUMBITS(2) [
            RW_EL1 = 0b00,
            RW_EL1_EL0 = 0b01,
            RO_EL1 = 0b10,
            RO_EL1_EL0 = 0b11
        ],

        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],

        TYPE OFFSET(1) NUMBITS(1) [
            Block = 0,
            Page = 1
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}
