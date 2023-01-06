use crate::{
    address::{Address, PhysicalAddress, VirtualAddress},
    address_map::{LOCAL_REGISTERS_BASE, LOCAL_REGISTERS_END, PERIPHERALS_BASE, PERIPHERALS_END},
    error::{Error, Result},
    kimage::{kernel_phy_range, kernel_stack_range},
};

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
    static ref KERNEL_VIRT_ADDRESS_BASE: VirtualAddress =
        VirtualAddress::new(0x0000_0000_0000_0000).unwrap();
}

/// Works only for statically mapped physical addresses
pub fn phy2virt(paddr: PhysicalAddress) -> Result<VirtualAddress> {
    let peripherals_range = PERIPHERALS_BASE..PERIPHERALS_END;
    let local_peripherals_range = LOCAL_REGISTERS_BASE..LOCAL_REGISTERS_END;
    let kernel_image_range = kernel_phy_range();
    let kernel_stack_range = kernel_stack_range();

    if !peripherals_range.contains(&paddr)
        && !local_peripherals_range.contains(&paddr)
        && !kernel_image_range.contains(&paddr)
        && !kernel_stack_range.contains(&paddr)
    {
        return Err(Error::PhysicalAddressNotStaticallyMapped(paddr));
    }

    Ok(*KERNEL_VIRT_ADDRESS_BASE + paddr.as_raw_ptr())
}
