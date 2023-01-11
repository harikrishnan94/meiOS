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
        VirtualAddress::new(0xFFFF_FFFF_0000_0000).unwrap();
    static ref EL0_VIRT_ADDRESS_BASE: VirtualAddress =
        VirtualAddress::new(0x0000_0000_0000_0000).unwrap();
}

/// Works only for statically mapped physical addresses
pub fn phy2virt(paddr: PhysicalAddress) -> VirtualAddress {
    *EL1_VIRT_ADDRESS_BASE + paddr.as_raw_ptr()
}

pub mod physical_page_alloc {
    use crate::{address::PhysicalAddress, error::Result};

    pub struct AllocationLayout {
        num_pages: usize,
        align: usize,
        is_contigious: bool,
    }

    impl AllocationLayout {
        pub fn new(num_pages: usize, align: usize, is_contigious: bool) -> Self {
            Self {
                num_pages,
                align,
                is_contigious,
            }
        }

        pub fn num_pages(&self) -> usize {
            self.num_pages
        }
        pub fn align(&self) -> usize {
            self.align
        }
        pub fn is_contigious(&self) -> bool {
            self.is_contigious
        }
    }

    pub struct PhysicalPages {
        phy_page_start: PhysicalAddress,
        num_allocated_pages: usize,
    }

    impl PhysicalPages {
        pub fn start_address(&self) -> PhysicalAddress {
            self.phy_page_start
        }

        pub fn allocated_page_count(&self) -> usize {
            self.num_allocated_pages
        }
    }

    pub trait Allocator {
        fn allocate_phy_pages(&mut self, layout: &AllocationLayout) -> Result<PhysicalPages>;

        fn free_phy_pages(
            &mut self,
            phy_page_start: PhysicalAddress,
            num_pages: usize,
        ) -> Result<()>;
    }
}

pub enum MemoryKind {
    /// DRAM memory: always cache-able.
    Normal,

    /// Device (Peripherals) memory: always non cache-able.
    Device,
}

/// Memory Map description of either a Normal or Device memory region
#[derive(Debug, Clone, Copy)]
pub enum MemoryMap {
    /// Normal (DRAM) Memory Region is always cache-able.
    Normal(MapDesc),

    /// Device Memory Region is always non cache-able.
    Device(MapDesc),
}

/// Describes a phy_addr -> virt_addr mapping of `num_pages` page count.
/// `len` is always aligned to GRANULE_SIZE.
#[derive(Debug, Clone, Copy)]
pub struct MapDesc {
    /// Page Aligned
    phy_addr: PhysicalAddress,
    /// Page Aligned
    virt_addr: VirtualAddress,
    num_pages: usize,
    access_perms: AccessPermissions,
}

impl MapDesc {
    pub fn physical_address(&self) -> PhysicalAddress {
        self.phy_addr
    }

    pub fn virtual_address(&self) -> VirtualAddress {
        self.virt_addr
    }

    pub fn num_pages(&self) -> usize {
        self.num_pages
    }

    pub fn access_permissions(&self) -> AccessPermissions {
        self.access_perms
    }
}

// AccessPermissions of a Mapped Region
bitflags! {
    pub struct AccessPermissions: u64 {
        // Access Permissions
        // EL1
        const EL1_READ = 0b0000_0001;
        const EL1_WRITE = 0b0000_0010;
        const EL1_EXECUTE = 0b0000_0101;
        // EL0
        const EL0_READ = 0b0001_0000;
        const EL0_WRITE = 0b0010_0000;
        const EL0_EXECUTE = 0b0101_0000;
    }
}

impl AccessPermissions {
    pub fn normal_memory_default() -> Self {
        AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE
    }

    pub fn device_memory_default() -> Self {
        AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE
    }

    pub fn user_memory_default() -> Self {
        AccessPermissions::normal_memory_default()
            | AccessPermissions::EL0_READ
            | AccessPermissions::EL0_WRITE
    }
}

impl core::fmt::Display for MemoryMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MemoryMap::Normal(desc) => write!(f, "Normal Memory Map: {desc}"),
            MemoryMap::Device(desc) => write!(f, "Device Memory Map: {desc}"),
        }
    }
}

impl core::fmt::Display for MapDesc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let phy_addr = &self.phy_addr;
        let virt_addr = &self.virt_addr;
        let num_pages = &self.num_pages;
        let access_perms = &self.access_perms;

        write!(
            f,
            "[phy_addr: {phy_addr}, virt_addr: {virt_addr}, num_pages: {num_pages}, access_permissions: {access_perms}]"
        )
    }
}

impl core::fmt::Display for AccessPermissions {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let bits = self.bits;
        write!(f, "{bits}")
    }
}
