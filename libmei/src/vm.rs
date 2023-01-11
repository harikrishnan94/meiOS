use crate::address::{Address, PhysicalAddress, VirtualAddress};

// From https://lwn.net/Articles/718895/
//
// AArch64 Linux memory layout with 4KB pages + 4 levels:
//
// Start                End                Size		Use
// -----------------------------------------------------------------------
// 0000000000000000	0000ffffffffffff	 256TB		user
// ffff000000000000	ffffffffffffffff	 256TB		kernel
//
// Translation table lookup with 4KB pages:
//
// +--------+--------+--------+--------+--------+--------+--------+--------+
// |63    56|55    48|47    40|39    32|31    24|23    16|15     8|7      0|
// +--------+--------+--------+--------+--------+--------+--------+--------+
//  |                 |         |         |         |         |
//  |                 |         |         |         |         v
//  |                 |         |         |         |   [11:0]  in-page offset
//  |                 |         |         |         +-> [20:12] L3 index
//  |                 |         |         +-----------> [29:21] L2 index
//  |                 |         +---------------------> [38:30] L1 index
//  |                 +-------------------------------> [47:39] L0 index
//  +-------------------------------------------------> [63] TTBR0/1

lazy_static! {
    static ref EL1_VIRT_ADDRESS_BASE: VirtualAddress =
        VirtualAddress::new(0x0000_0000_0000_0000).unwrap();
}

/// Works only for statically mapped physical addresses
pub fn phy2virt(paddr: PhysicalAddress) -> VirtualAddress {
    *EL1_VIRT_ADDRESS_BASE + paddr.as_raw_ptr()
}
