//! There are 2 types of translation tables implemented.
//! 1) Static Translation table:
//!     - Maps the entire physical memory (normal and device) region into the low end of Kernel Space.
//!     - This is a static map, which doesn't allow changes to it.
//!     - This is Loaded into TTBR1 and is used in Privileged (Kernel) mode.
//!     - Kernel needs access to entire physical address space, so this is justified.
//! 2) Dynamic Translation table:
//!     - Mappings can be created/modified/destroyed dynamically
//!     - Supports splitting/merging adjacent mappings.
//!     - This is loaded into TTBR0 and is used in Un-privileged (User) mode.

use core::{alloc::Layout, cell::UnsafeCell, cmp::min, mem::size_of};

use tock_registers::{
    interfaces::{ReadWriteable, Readable},
    registers::InMemoryRegister,
};

use crate::{
    address::{Address, AddressTranslationLevel, PhysicalAddress, VirtualAddress},
    bug,
    error::{Error, Result},
    mmu::NEXT_LEVEL_TABLE_ADDR_SHIFT,
    vm::{AccessPermissions, MemoryKind, MemoryMap, PhysicalPageAllocator},
};

use super::{
    GRANULE_SIZE, LEVEL_1_OUTPUT_ADDR_SHIFT, LEVEL_2_OUTPUT_ADDR_SHIFT, LEVEL_3_OUTPUT_ADDR_SHIFT,
    STAGE1_BLOCK_DESCRIPTOR, STAGE1_PAGE_DESCRIPTOR, STAGE1_TABLE_DESCRIPTOR,
    TRANSLATION_TABLE_DESC_ALIGN,
};

const NUM_TABLE_DESC_ENTRIES: usize = 512;
const INVALID_DESCRIPTOR: u64 = 0;
const TRANSLATION_LEVELS: &[AddressTranslationLevel] = &[
    AddressTranslationLevel::Zero,
    AddressTranslationLevel::One,
    AddressTranslationLevel::Two,
    AddressTranslationLevel::Three,
];
const ONE_GIB: usize = 1024 * 1024 * 1024;
const TWO_MIB: usize = 2 * 1024 * 1024;
const FOUR_KIB: usize = 4 * 1024;

type Stage1PageDescriptor = InMemoryRegister<u64, STAGE1_PAGE_DESCRIPTOR::Register>;
type Stage1TableDescriptor = InMemoryRegister<u64, STAGE1_TABLE_DESCRIPTOR::Register>;
type Stage1BlockDescriptor = InMemoryRegister<u64, STAGE1_BLOCK_DESCRIPTOR::Register>;

/// Translation Table Descriptors
#[derive(Debug)]
#[repr(C)]
#[repr(align(4096))]
struct DescriptorTable(UnsafeCell<[u64; NUM_TABLE_DESC_ENTRIES]>);

impl Default for DescriptorTable {
    fn default() -> Self {
        Self(UnsafeCell::new(
            [INVALID_DESCRIPTOR; NUM_TABLE_DESC_ENTRIES],
        ))
    }
}

/// This stores the root of Translation Table
/// Address of `root` is stored in TTBR0/1.
///
/// ### Notes on alignment requirements on Virtual and Physical Address space:
///
/// Starting from level 1, Virtual Address and Physical address are aligned to same boundary
/// (i.e):
/// In level 1, with Level 1 Block descriptor, VA and PA both are aligned to 1 GiB boundary.
/// In level 2, with Level 2 Block descriptor, VA and PA both are aligned to 2 MiB boundary.
/// In level 3, with Page Descriptor, VA and PA both are aligned to 4 KiB boundary.
///
/// This means, Huge Pages must be aligned at both Virtual and Physical address spaces.
/// Consequently, if either of the address'es are unaligned to the required huge page boundary (1GiB/2MiB),
/// then huge pages of that size cannot be mapped. Must go with lower sized pages.
///
/// Though, an 1 GiB VA mapping consisting of 512 2MiB PA pages is only needed to be aligned at 2MiB boundary.
/// Similarly, a 2 MiB VA mapping consisting of 512 4KiB PA pages is only needed to be aligned at 4KiB boundary.
#[derive(Default)]
pub struct TranslationTable {
    root: DescriptorTable,
}

impl TranslationTable {
    pub fn new<DescAlloc: PhysicalPageAllocator>(
        maps: &[MemoryMap],
        desc_alloc: &DescAlloc,
    ) -> Result<Self> {
        let tt = Self {
            root: DescriptorTable::default(),
        };

        for map in maps {
            tt.map_impl(&parse_memory_map(map), desc_alloc, map)?;
        }

        Ok(tt)
    }

    /// Add Mapping to translation table
    pub fn map<DescAlloc: PhysicalPageAllocator>(
        &self,
        map: &MemoryMap,
        desc_alloc: &DescAlloc,
    ) -> Result<()> {
        self.map_impl(&parse_memory_map(map), desc_alloc, map)
    }

    /// Walk the translation table using the VirtualAddress `vaddr` and produce corresponding PhysicalAddress
    /// This is similar to what CPU does after a TLB Miss.
    pub fn virt2phy(&self, vaddr: VirtualAddress) -> Option<TranslationDesc> {
        // #[cfg(test)]
        // print!("Translating vaddr {vaddr}...");
        let mut descs = &self.root;

        for level in TRANSLATION_LEVELS {
            let idx = vaddr.get_idx_for_level(level);
            let desc = load_desc(descs, idx);

            // #[cfg(test)]
            // print!(
            //     "Visiting 0x{:X}[{idx}] at level {level:?}...",
            //     descs.0.as_ptr() as u64
            // );

            match parse_desc(desc, level).ok()? {
                Descriptor::Table(tbl_desc) => {
                    // #[cfg(test)]
                    // print!("Found TBL Desc: 0x{:X}...", tbl_desc.get());
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(block_desc) => {
                    let create_translation_desc = |phy_addr: usize| {
                        let is_cacheable =
                            !block_desc.matches_all(STAGE1_BLOCK_DESCRIPTOR::SH::OuterShareable);

                        TranslationDesc {
                            virt_addr: vaddr,
                            phy_addr: PhysicalAddress::new(phy_addr),
                            access_perms: parse_access_perms_bd(&block_desc),
                            memory_kind: if is_cacheable {
                                MemoryKind::Normal
                            } else {
                                MemoryKind::Device
                            },
                        }
                    };

                    let output_address;
                    let page_offset;
                    let phy_addr;

                    match BlockDescLevel::from(level) {
                        BlockDescLevel::One => {
                            // #[cfg(test)]
                            // print!("Found L1 Block Desc: 0x{:X}...", block_desc.get());
                            output_address =
                                block_desc.read(STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_1GiB) as usize;
                            page_offset = vaddr.get_page_offset_1GiB();
                            phy_addr = (output_address << LEVEL_1_OUTPUT_ADDR_SHIFT) | page_offset;
                        }
                        BlockDescLevel::Two => {
                            // #[cfg(test)]
                            // print!("Found L2 Block Desc: 0x{:X}...", block_desc.get());
                            output_address =
                                block_desc.read(STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_2MiB) as usize;
                            page_offset = vaddr.get_page_offset_2MiB();
                            phy_addr = (output_address << LEVEL_2_OUTPUT_ADDR_SHIFT) | page_offset;
                        }
                    }

                    return Some(create_translation_desc(phy_addr));
                }
                Descriptor::Page(page_desc) => {
                    // #[cfg(test)]
                    // print!("Found Page Desc: 0x{:X}...", page_desc.get());

                    let output_address =
                        page_desc.read(STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB) as usize;
                    let page_offset = vaddr.get_page_offset_4KiB();
                    let phy_addr = (output_address << LEVEL_3_OUTPUT_ADDR_SHIFT) | page_offset;
                    let is_cacheable =
                        !page_desc.matches_all(STAGE1_PAGE_DESCRIPTOR::SH::OuterShareable);

                    return Some(TranslationDesc {
                        virt_addr: vaddr,
                        phy_addr: PhysicalAddress::new(phy_addr),
                        access_perms: parse_access_perms_pd(&page_desc),
                        memory_kind: if is_cacheable {
                            MemoryKind::Normal
                        } else {
                            MemoryKind::Device
                        },
                    });
                }
                Descriptor::Invalid => {
                    // #[cfg(test)]
                    // print!("Found Invalid Desc: 0x{desc:X}...");
                    return None;
                }
            }
            // #[cfg(test)]
            // std::io::Write::flush(&mut std::io::stdout());
        }

        bug!("Cannot reach here");
    }

    pub fn get_base_address(&self) -> u64 {
        self.root.0.get() as u64
    }

    fn map_impl<DescAlloc: PhysicalPageAllocator>(
        &self,
        map: &ParsedMemoryMap,
        desc_alloc: &DescAlloc,
        mmap: &MemoryMap,
    ) -> Result<()> {
        let map_scheme =
            determine_mapping_scheme(map.virt_addr, map.phy_addr, map.num_pages * GRANULE_SIZE);
        let mut map = ParsedMemoryMap {
            phy_addr: map.phy_addr,
            virt_addr: map.virt_addr,
            attributes: map.attributes,
            num_pages: 0,
        };

        // #[cfg(test)]
        // print!(
        //     "Mapping paddr {} to vaddr {}...",
        //     map.phy_addr, map.virt_addr
        // );

        map.num_pages = map_scheme.four_kib_aligned_span;
        while map.num_pages > 0 {
            self.install_page_descs(&mut map, desc_alloc, mmap)
                .map_err(|e| {
                    // #[cfg(test)]
                    // println!("{e:?}");
                    e
                })?;
            // #[cfg(test)]
            // println!("Done")
        }

        map.num_pages = map_scheme.two_mib_aligned_span;
        while map.num_pages > 0 {
            self.install_l2_block_desc(&mut map, desc_alloc, mmap)
                .map_err(|e| {
                    // #[cfg(test)]
                    // println!("{e:?}");
                    e
                })?;
            // #[cfg(test)]
            // println!("Done")
        }

        map.num_pages = map_scheme.one_gib_aligned_span;
        while map.num_pages > 0 {
            self.install_l1_block_desc(&mut map, desc_alloc, mmap)
                .map_err(|e| {
                    // #[cfg(test)]
                    // println!("{e:?}");
                    e
                })?;
            // #[cfg(test)]
            // println!("Done")
        }

        Ok(())
    }

    fn install_page_descs<DescAlloc: PhysicalPageAllocator>(
        &self,
        map: &mut ParsedMemoryMap,
        desc_alloc: &DescAlloc,
        mmap: &MemoryMap,
    ) -> Result<()> {
        let mut descs = &self.root;

        for level in TRANSLATION_LEVELS {
            let idx = map.virt_addr.get_idx_for_level(level);
            let desc = load_desc(descs, idx);

            // #[cfg(test)]
            // print!(
            //     "Visiting 0x{:X}[{idx}] at level {level:?}...",
            //     descs.0.as_ptr() as u64
            // );

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    // #[cfg(test)]
                    // print!("Found TBL Desc: 0x{:X}...", tbl_desc.get());
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) | Descriptor::Page(_) => {
                    // #[cfg(test)]
                    // print!("Found Block Or Page Desc: 0x{desc:X}...");
                    return Err(Error::VMMapExists(*mmap));
                }
                Descriptor::Invalid => {
                    // #[cfg(test)]
                    // print!("Found Invalid Desc: 0x{desc:X}...");
                    match level {
                        // We need to insert only Page Descriptor.
                        // Until we reach level 3, insert Table Descriptors.
                        AddressTranslationLevel::Zero
                        | AddressTranslationLevel::One
                        | AddressTranslationLevel::Two => {
                            let tbl_desc =
                                install_new_tbl_desc(desc_alloc, load_desc_mut(descs, idx))?;
                            descend_tbl_desc(tbl_desc, &mut descs);
                        }
                        AddressTranslationLevel::Three => {
                            install_contigious_mappings(
                                map,
                                idx,
                                descs,
                                FOUR_KIB,
                                &|output_address, attributes| {
                                    new_stage1_page_desc(output_address, attributes)
                                },
                            );
                            break;
                        }
                    }
                }
            }
            // #[cfg(test)]
            // std::io::Write::flush(&mut std::io::stdout());
        }

        Ok(())
    }

    fn install_l2_block_desc<DescAlloc: PhysicalPageAllocator>(
        &self,
        map: &mut ParsedMemoryMap,
        desc_alloc: &DescAlloc,
        mmap: &MemoryMap,
    ) -> Result<()> {
        let mut descs = &self.root;

        for level in TRANSLATION_LEVELS {
            let idx = map.virt_addr.get_idx_for_level(level);
            let desc = load_desc(descs, idx);

            // #[cfg(test)]
            // print!(
            //     "Visiting 0x{:X}[{idx}] at level {level:?}...",
            //     descs.0.as_ptr() as u64
            // );

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    // #[cfg(test)]
                    // print!("Found TBL Desc: 0x{:X}...", tbl_desc.get());
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) => {
                    // #[cfg(test)]
                    // print!("Found Block Desc: 0x{desc:X}...");
                    return Err(Error::VMMapExists(*mmap));
                }
                Descriptor::Page(_) => {
                    // #[cfg(test)]
                    // print!("Found Page Desc: 0x{desc:X}...");
                    return Err(Error::CorruptedTranslationTable(desc));
                }
                Descriptor::Invalid => {
                    // #[cfg(test)]
                    // print!("Found Invalid Desc: 0x{desc:X}...");
                    // We need to insert only Level 2 Block Descriptor.
                    // Until we reach level 2, insert Table Descriptors.
                    match level {
                        AddressTranslationLevel::Zero | AddressTranslationLevel::One => {
                            let tbl_desc =
                                install_new_tbl_desc(desc_alloc, load_desc_mut(descs, idx))?;
                            descend_tbl_desc(tbl_desc, &mut descs);
                        }
                        AddressTranslationLevel::Two => {
                            install_contigious_mappings(
                                map,
                                idx,
                                descs,
                                TWO_MIB,
                                &|output_address, attributes| {
                                    new_stage1_block_desc(
                                        BlockDescLevel::Two,
                                        output_address,
                                        attributes,
                                    )
                                },
                            );
                            break;
                        }
                        AddressTranslationLevel::Three => {
                            return Err(Error::CorruptedTranslationTable(desc))
                        }
                    }
                }
            }
            // #[cfg(test)]
            // std::io::Write::flush(&mut std::io::stdout());
        }

        Ok(())
    }

    fn install_l1_block_desc<DescAlloc: PhysicalPageAllocator>(
        &self,
        map: &mut ParsedMemoryMap,
        desc_alloc: &DescAlloc,
        mmap: &MemoryMap,
    ) -> Result<()> {
        let mut descs = &self.root;

        for level in TRANSLATION_LEVELS {
            let idx = map.virt_addr.get_idx_for_level(level);
            let desc = load_desc(descs, idx);

            // #[cfg(test)]
            // print!(
            //     "Visiting 0x{:X}[{idx}] at level {level:?}...",
            //     descs.0.as_ptr() as u64
            // );

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    // #[cfg(test)]
                    // print!("Found TBL Desc: 0x{:X}...", tbl_desc.get());
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) => {
                    if *level == AddressTranslationLevel::One {
                        // #[cfg(test)]
                        // print!("Found L1 Block Desc: 0x{desc:X}...");
                        return Err(Error::VMMapExists(*mmap));
                    } else {
                        // #[cfg(test)]
                        // print!("Found L2 Block Desc: 0x{desc:X}...");
                        return Err(Error::CorruptedTranslationTable(desc));
                    }
                }
                Descriptor::Page(_) => {
                    // #[cfg(test)]
                    // print!("Found Page Desc: 0x{desc:X}...");
                    return Err(Error::CorruptedTranslationTable(desc));
                }
                Descriptor::Invalid => {
                    // #[cfg(test)]
                    // print!("Found Invalid Desc: 0x{desc:X}...");
                    // We need to insert only Level 1 Block Descriptor.
                    // Until we reach level 1, insert Table Descriptors.
                    match level {
                        AddressTranslationLevel::Zero => {
                            let tbl_desc =
                                install_new_tbl_desc(desc_alloc, load_desc_mut(descs, idx))?;
                            descend_tbl_desc(tbl_desc, &mut descs);
                        }
                        AddressTranslationLevel::One => {
                            install_contigious_mappings(
                                map,
                                idx,
                                descs,
                                ONE_GIB,
                                &|output_address, attributes| {
                                    new_stage1_block_desc(
                                        BlockDescLevel::One,
                                        output_address,
                                        attributes,
                                    )
                                },
                            );
                            break;
                        }
                        AddressTranslationLevel::Two | AddressTranslationLevel::Three => {
                            return Err(Error::CorruptedTranslationTable(desc))
                        }
                    }
                }
            }
            // #[cfg(test)]
            // std::io::Write::flush(&mut std::io::stdout());
        }

        Ok(())
    }
}

fn read_next_level_desc(tbl_desc: &Stage1TableDescriptor) -> u64 {
    tbl_desc.read(STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR) << NEXT_LEVEL_TABLE_ADDR_SHIFT
}

fn get_next_level_desc<'tt>(tbl_desc: &Stage1TableDescriptor) -> &'tt DescriptorTable {
    let next_lvl_desc = read_next_level_desc(&tbl_desc);
    assert_ne!(next_lvl_desc, 0);
    // #[cfg(test)]
    // print!("descending to 0x{next_lvl_desc:X}...");
    unsafe { &*(next_lvl_desc as *mut DescriptorTable) }
}

fn read_next_level_desc(tbl_desc: &Stage1TableDescriptor) -> u64 {
    tbl_desc.read(STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR) << NEXT_LEVEL_TABLE_ADDR_SHIFT
}

fn descend_tbl_desc(tbl_desc: Stage1TableDescriptor, descs: &mut &DescriptorTable) {
    *descs = get_next_level_desc(&tbl_desc);
}

fn install_new_tbl_desc<DescAlloc: PhysicalPageAllocator>(
    desc_alloc: &DescAlloc,
    new_desc: &mut u64,
) -> Result<Stage1TableDescriptor> {
    let alloc_desc_table = || -> Result<u64> {
        let layout =
            Layout::from_size_align(size_of::<DescriptorTable>(), TRANSLATION_TABLE_DESC_ALIGN)
                .unwrap_or_else(|_| bug!("Descriptor Layout Mismatch"));
        let ptr = desc_alloc
            .allocate_zeroed(layout)
            .map_err(|_| Error::PhysicalOOM)?
            .as_non_null_ptr()
            .addr()
            .get();
        Ok(ptr as u64)
    };
    let next_level_table = alloc_desc_table()?;
    // #[cfg(test)]
    // print!("allocating TBL Desc 0x{next_level_table:X}...");
    let tbl_desc = Stage1TableDescriptor::new(new_stage1_table_desc(next_level_table));
    *new_desc = tbl_desc.get();
    Ok(tbl_desc)
}

fn install_contigious_mappings<F: Fn(u64, u64) -> u64>(
    map: &mut ParsedMemoryMap,
    idx: usize,
    descs: &DescriptorTable,
    page_size: usize,
    new_stage1_descriptor: &F,
) {
    let mut paddr = map.phy_addr.as_raw_ptr() as u64;
    let num_mapped_pages = core::cmp::min(map.num_pages, NUM_TABLE_DESC_ENTRIES - idx);
    for i in 0..num_mapped_pages {
        assert_eq!(load_desc(descs, idx + i), INVALID_DESCRIPTOR);
        let desc = new_stage1_descriptor(paddr, map.attributes);
        *load_desc_mut(descs, idx + i) = desc;
        paddr += page_size as u64;
    }
    map.phy_addr += num_mapped_pages * page_size;
    map.virt_addr += num_mapped_pages * page_size;
    map.num_pages -= num_mapped_pages;

    // #[cfg(test)]
    // print!("Installing {num_mapped_pages} mappings of size: {page_size}...",);
}

#[derive(Default, Clone, Copy)]
struct MappingScheme {
    /// Number of Pages in 4KiB boundary
    four_kib_aligned_span: usize,
    /// Number of Pages in 2MiB boundary
    two_mib_aligned_span: usize,
    /// Number of Pages in 1GiB boundary
    one_gib_aligned_span: usize,
}

const ALIGNMENTS: [usize; 3] = [ONE_GIB, TWO_MIB, FOUR_KIB];

fn determine_mapping_scheme_impl(
    vaddr: VirtualAddress,
    paddr: PhysicalAddress,
    mut size: usize,
    start_ind: usize,
) -> MappingScheme {
    assert!(start_ind < ALIGNMENTS.len());

    let mut mapping_scheme = MappingScheme::default();

    for (i, align) in ALIGNMENTS[start_ind..].iter().enumerate() {
        let offset = paddr.align_offset(*align);

        if vaddr.align_offset(*align) == offset {
            if offset != 0 {
                let peeled_size = min(offset, size);
                // Recursive call will only set the lower alignment boundary page counts.
                mapping_scheme =
                    determine_mapping_scheme_impl(vaddr, paddr, peeled_size, start_ind + i + 1);
                size -= peeled_size;
            }

            // This will only set the current alignment boundary page count.
            let page_count = size / align;
            match *align {
                ONE_GIB => mapping_scheme.one_gib_aligned_span = page_count,
                TWO_MIB => mapping_scheme.two_mib_aligned_span = page_count,
                FOUR_KIB => mapping_scheme.four_kib_aligned_span = page_count,
                _ => bug!("invalid alignment"),
            };
            size -= page_count * *align;

            if size == 0 {
                break;
            }
        }
    }

    mapping_scheme
}

fn determine_mapping_scheme(
    vaddr: VirtualAddress,
    paddr: PhysicalAddress,
    size: usize,
) -> MappingScheme {
    assert!(vaddr.is_aligned(GRANULE_SIZE));
    assert!(paddr.is_aligned(GRANULE_SIZE));

    determine_mapping_scheme_impl(vaddr, paddr, size, 0)
}

pub struct TranslationDesc {
    virt_addr: VirtualAddress,
    phy_addr: PhysicalAddress,
    access_perms: AccessPermissions,
    memory_kind: MemoryKind,
}

struct ParsedMemoryMap {
    /// Page Aligned
    phy_addr: PhysicalAddress,
    /// Page Aligned
    virt_addr: VirtualAddress,
    num_pages: usize,
    attributes: u64,
}

fn parse_memory_map(map: &MemoryMap) -> ParsedMemoryMap {
    match map {
        MemoryMap::Normal(desc) => ParsedMemoryMap {
            phy_addr: desc.physical_address(),
            virt_addr: desc.virtual_address(),
            num_pages: desc.num_pages(),
            attributes: parse_map_attrs(&desc.access_permissions(), MemoryKind::Normal),
        },
        MemoryMap::Device(desc) => ParsedMemoryMap {
            phy_addr: desc.physical_address(),
            virt_addr: desc.virtual_address(),
            num_pages: desc.num_pages(),
            attributes: parse_map_attrs(&desc.access_permissions(), MemoryKind::Device),
        },
    }
}

fn parse_map_attrs(ap: &AccessPermissions, device: MemoryKind) -> u64 {
    let page_desc = Stage1PageDescriptor::new(0);
    let el1_rw = ap.contains(AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE);
    let el0_rw = ap.contains(AccessPermissions::EL0_READ | AccessPermissions::EL0_WRITE);
    let el1_ro = ap.contains(AccessPermissions::EL1_READ);
    let el0_ro = ap.contains(AccessPermissions::EL0_READ);

    if el1_rw {
        if el0_rw {
            page_desc.modify(STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1_EL0)
        } else {
            page_desc.modify(STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1)
        }
    } else if el1_ro {
        if el0_ro {
            page_desc.modify(STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1_EL0)
        } else {
            page_desc.modify(STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1)
        }
    }

    if ap.contains(AccessPermissions::EL1_WRITE) || !ap.contains(AccessPermissions::EL1_EXECUTE) {
        page_desc.modify(STAGE1_PAGE_DESCRIPTOR::PXN::SET);
    }
    if ap.contains(AccessPermissions::EL0_WRITE) || !ap.contains(AccessPermissions::EL0_EXECUTE) {
        page_desc.modify(STAGE1_PAGE_DESCRIPTOR::UXN::SET);
    }

    match device {
        MemoryKind::Normal => page_desc.modify(STAGE1_PAGE_DESCRIPTOR::SH::InnerShareable),
        MemoryKind::Device => page_desc.modify(STAGE1_PAGE_DESCRIPTOR::SH::OuterShareable),
    }

    page_desc.get()
}

enum Descriptor {
    Table(Stage1TableDescriptor),
    Block(Stage1BlockDescriptor),
    Page(Stage1PageDescriptor),
    Invalid,
}

enum RawDescriptor {
    TableOrPage(u64),
    Block(Stage1BlockDescriptor),
    Invalid,
}

fn parse_desc(
    desc_val: u64,
    level: &AddressTranslationLevel,
) -> core::result::Result<Descriptor, Descriptor> {
    match to_raw_desc(desc_val) {
        RawDescriptor::TableOrPage(desc_val) => {
            if level == &AddressTranslationLevel::Three {
                // PageDescriptors can be present only in last level (3).
                Ok(Descriptor::Page(Stage1PageDescriptor::new(desc_val)))
            } else {
                // TableDescriptors can appear at any level other than last level (3).
                Ok(Descriptor::Table(Stage1TableDescriptor::new(desc_val)))
            }
        }
        RawDescriptor::Block(block_desc) => {
            // Block Descriptors can be present only in levels 1 and 2.
            if level == &AddressTranslationLevel::One || level == &AddressTranslationLevel::Two {
                Ok(Descriptor::Block(block_desc))
            } else {
                Err(Descriptor::Block(block_desc))
            }
        }
        // Invalid Descriptors can appear anywhere.
        RawDescriptor::Invalid => Ok(Descriptor::Invalid),
    }
}

fn to_raw_desc(value: u64) -> RawDescriptor {
    // Table and Page descriptors both have bits [0:1] set to 0b11.
    // Only way they can be distinguished is at which level they appear.
    // So, treat them as same.
    let tbl_desc = Stage1TableDescriptor::new(value);
    if tbl_desc
        .matches_all(STAGE1_TABLE_DESCRIPTOR::VALID::SET + STAGE1_TABLE_DESCRIPTOR::TYPE::Table)
    {
        return RawDescriptor::TableOrPage(value);
    }

    let block_desc = Stage1BlockDescriptor::new(value);
    if block_desc
        .matches_all(STAGE1_BLOCK_DESCRIPTOR::VALID::SET + STAGE1_BLOCK_DESCRIPTOR::TYPE::Block)
    {
        return RawDescriptor::Block(block_desc);
    }

    RawDescriptor::Invalid
}

fn parse_access_perms_bd(block_desc: &Stage1BlockDescriptor) -> AccessPermissions {
    let mut access_perms = parse_access_perms(block_desc.read(STAGE1_BLOCK_DESCRIPTOR::AP));

    if !block_desc.is_set(STAGE1_BLOCK_DESCRIPTOR::PXN)
        && !access_perms.contains(AccessPermissions::EL1_WRITE)
    {
        access_perms |= AccessPermissions::EL1_EXECUTE;
    }
    if !block_desc.is_set(STAGE1_BLOCK_DESCRIPTOR::UXN)
        && !access_perms.contains(AccessPermissions::EL0_WRITE)
    {
        access_perms |= AccessPermissions::EL0_EXECUTE;
    }

    access_perms
}

fn parse_access_perms_pd(page_desc: &Stage1PageDescriptor) -> AccessPermissions {
    let mut access_perms = parse_access_perms(page_desc.read(STAGE1_PAGE_DESCRIPTOR::AP));

    if !page_desc.is_set(STAGE1_PAGE_DESCRIPTOR::PXN)
        && !access_perms.contains(AccessPermissions::EL1_WRITE)
    {
        access_perms |= AccessPermissions::EL1_EXECUTE;
    }
    if !page_desc.is_set(STAGE1_PAGE_DESCRIPTOR::UXN)
        && !access_perms.contains(AccessPermissions::EL0_WRITE)
    {
        access_perms |= AccessPermissions::EL0_EXECUTE;
    }

    access_perms
}

fn parse_access_perms(ap: u64) -> AccessPermissions {
    if ap == STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1_EL0.value {
        AccessPermissions::EL1_READ
            | AccessPermissions::EL1_WRITE
            | AccessPermissions::EL0_READ
            | AccessPermissions::EL0_WRITE
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1_EL0.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL0_READ
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1.value {
        AccessPermissions::EL1_READ
    } else {
        bug!("Invalid Access Permissions on page");
    }
}

fn new_stage1_table_desc(next_level_addr: u64) -> u64 {
    let table_desc = Stage1TableDescriptor::new(0);

    assert_ne!(next_level_addr, 0);
    assert_eq!(
        next_level_addr & ((1 << NEXT_LEVEL_TABLE_ADDR_SHIFT) - 1),
        0
    );

    table_desc.modify(STAGE1_TABLE_DESCRIPTOR::VALID::True + STAGE1_TABLE_DESCRIPTOR::TYPE::Table);
    table_desc.modify(
        STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR
            .val(next_level_addr >> NEXT_LEVEL_TABLE_ADDR_SHIFT),
    );

    table_desc.get()
}

fn new_stage1_page_desc(output_address: u64, attributes: u64) -> u64 {
    let page_desc = Stage1PageDescriptor::new(attributes);

    assert_eq!(output_address & ((1 << LEVEL_3_OUTPUT_ADDR_SHIFT) - 1), 0);

    page_desc.modify(STAGE1_PAGE_DESCRIPTOR::VALID::True + STAGE1_PAGE_DESCRIPTOR::TYPE::Page);
    page_desc.modify(
        STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB.val(output_address >> LEVEL_3_OUTPUT_ADDR_SHIFT),
    );

    page_desc.get()
}

enum BlockDescLevel {
    One,
    Two,
}

impl BlockDescLevel {
    fn from(level: &AddressTranslationLevel) -> Self {
        match level {
            AddressTranslationLevel::One => Self::One,
            AddressTranslationLevel::Two => Self::Two,
            _ => bug!("Wrong Level used for constructing a Block Descriptor"),
        }
    }
}

fn new_stage1_block_desc(level: BlockDescLevel, output_address: u64, attributes: u64) -> u64 {
    let block_desc = Stage1BlockDescriptor::new(attributes);

    block_desc.modify(STAGE1_BLOCK_DESCRIPTOR::VALID::True + STAGE1_BLOCK_DESCRIPTOR::TYPE::Block);

    match level {
        BlockDescLevel::One => {
            assert_eq!(output_address & ((1 << LEVEL_1_OUTPUT_ADDR_SHIFT) - 1), 0);
            block_desc.modify(
                STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_1GiB
                    .val(output_address >> LEVEL_1_OUTPUT_ADDR_SHIFT),
            )
        }
        BlockDescLevel::Two => {
            assert_eq!(output_address & ((1 << LEVEL_2_OUTPUT_ADDR_SHIFT) - 1), 0);
            block_desc.modify(
                STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_2MiB
                    .val(output_address >> LEVEL_2_OUTPUT_ADDR_SHIFT),
            )
        }
    }

    block_desc.get()
}

fn load_desc(descs: &DescriptorTable, idx: usize) -> u64 {
    unsafe { (*descs.0.get())[idx] }
}

fn load_desc_mut(descs: &DescriptorTable, idx: usize) -> &mut u64 {
    unsafe { &mut (*descs.0.get())[idx] }
}

#[cfg(all(test, not(feature = "no_std")))]
mod tests {
    extern crate std;

    use core::{
        alloc::{AllocError, Allocator, Layout},
        cell::RefCell,
        ptr::NonNull,
    };
    use rand::{seq::SliceRandom, thread_rng, Rng};
    use rayon::prelude::*;
    use std::{collections::HashMap, println, vec::Vec};

    use crate::{
        address::{PhysicalAddress, VirtualAddress},
        mmu::{
            translation_table::{TranslationTable, NUM_TABLE_DESC_ENTRIES},
            GRANULE_SIZE, OUTPUT_ADDR_BITS,
        },
        vm::{AccessPermissions, MapDesc, MemoryKind, MemoryMap, PhysicalPageAllocator},
    };

    use super::{determine_mapping_scheme, FOUR_KIB, ONE_GIB, TWO_MIB};

    #[derive(Default)]
    struct TestAllocator {
        mem: RefCell<HashMap<*mut u8, Layout>>,
    }

    unsafe impl Allocator for TestAllocator {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            let ptr = unsafe { std::alloc::alloc(layout) };
            self.mem.borrow_mut().insert(ptr, layout);
            unsafe {
                Ok(NonNull::slice_from_raw_parts(
                    NonNull::new_unchecked(ptr),
                    layout.size(),
                ))
            }
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            let ptr = ptr.addr().get() as *mut u8;
            self.mem.borrow_mut().remove(&ptr);
            unsafe { std::alloc::dealloc(ptr, layout) };
        }
    }

    impl Drop for TestAllocator {
        fn drop(&mut self) {
            for (ptr, layout) in self.mem.borrow().iter() {
                unsafe { std::alloc::dealloc(*ptr, *layout) };
            }
        }
    }

    impl PhysicalPageAllocator for TestAllocator {}

    #[warn(non_snake_case)]
    fn get_a_random_512GiB_range() -> u32 {
        thread_rng().gen_range(0..NUM_TABLE_DESC_ENTRIES) as u32
    }

    fn get_random_range(start: u32, end: u32) -> Vec<u32> {
        let mut vec: Vec<u32> = (start..end).collect();
        vec.shuffle(&mut thread_rng());
        vec
    }

    fn get_random_virt_addr() -> VirtualAddress {
        const TOTAL_VIRTUAL_ADDRESS_SPACE: usize = 1usize << OUTPUT_ADDR_BITS;
        VirtualAddress::new(
            rand::thread_rng().gen_range(0..(TOTAL_VIRTUAL_ADDRESS_SPACE / ONE_GIB) - 1) * ONE_GIB,
        )
        .unwrap()
    }

    fn generate_memory_maps(mut virt_addr: VirtualAddress) -> Vec<MemoryMap> {
        let rand_1GiB_ranges = get_random_range(0, NUM_TABLE_DESC_ENTRIES as u32);
        let rand_2MiB_ranges = get_random_range(0, NUM_TABLE_DESC_ENTRIES as u32);
        let rand_4KiB_ranges = get_random_range(0, NUM_TABLE_DESC_ENTRIES as u32);
        let access_perms = AccessPermissions::normal_memory_default();
        let mut memory_maps = Vec::new();
        let form_phy_addr = |OneGiB: u32, TwoMiB: u32, FourKiB| {
            PhysicalAddress::new(
                OneGiB as usize * ONE_GIB + TwoMiB as usize * TWO_MIB + FourKiB as usize * FOUR_KIB,
            )
        };

        for (i, one_gib_ind) in rand_1GiB_ranges.iter().enumerate() {
            if i == NUM_TABLE_DESC_ENTRIES - 1 {
                for (i, two_mib_ind) in rand_2MiB_ranges.iter().enumerate() {
                    if i == NUM_TABLE_DESC_ENTRIES - 1 {
                        for four_kib_ind in &rand_4KiB_ranges {
                            memory_maps.push(MemoryMap::Normal(MapDesc::new(
                                form_phy_addr(*one_gib_ind, *two_mib_ind, *four_kib_ind),
                                virt_addr,
                                FOUR_KIB / GRANULE_SIZE,
                                access_perms,
                            )));

                            virt_addr += FOUR_KIB;
                        }
                    } else {
                        memory_maps.push(MemoryMap::Normal(MapDesc::new(
                            form_phy_addr(*one_gib_ind, *two_mib_ind, 0),
                            virt_addr,
                            TWO_MIB / GRANULE_SIZE,
                            access_perms,
                        )));

                        virt_addr += TWO_MIB;
                    }
                }
            } else {
                memory_maps.push(MemoryMap::Normal(MapDesc::new(
                    form_phy_addr(*one_gib_ind, 0, 0),
                    virt_addr,
                    ONE_GIB / GRANULE_SIZE,
                    access_perms,
                )));
            }

            virt_addr += ONE_GIB;
        }

        memory_maps.shuffle(&mut thread_rng());
        memory_maps
    }

    fn insert_test_using_vaddr(vaddr: VirtualAddress) {
        let page_alloc = TestAllocator::default();
        let memory_maps = generate_memory_maps(vaddr);
        let translation_table = TranslationTable::new(&memory_maps, &page_alloc);

        assert!(translation_table.is_ok());

        let translation_table = translation_table.unwrap();

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let translation = translation_table.virt2phy(vaddr);
                    // println!("");

                    assert!(translation.is_some());

                    let translation = translation.unwrap();

                    assert_eq!(translation.phy_addr, desc.physical_address());
                    assert_eq!(translation.access_perms, desc.access_permissions());
                    assert_eq!(translation.memory_kind, MemoryKind::Normal);
                }
                MemoryMap::Device(_) => assert!(false, "Failure"),
            }
        }
    }

    fn mapping_scheme_test_using_vaddr(vaddr: VirtualAddress) {
        let page_alloc = TestAllocator::default();
        let memory_maps = generate_memory_maps(vaddr);

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let paddr = desc.physical_address();
                    let size = desc.num_pages() * GRANULE_SIZE;
                    let scheme = determine_mapping_scheme(vaddr, paddr, size);

                    assert_eq!(
                        scheme.four_kib_aligned_span * FOUR_KIB
                            + scheme.two_mib_aligned_span * TWO_MIB
                            + scheme.one_gib_aligned_span * ONE_GIB,
                        size
                    );
                }
                MemoryMap::Device(_) => assert!(false, "Failure"),
            }
        }
    }

    #[test]
    fn mapping_scheme_test() {
        let vaddr = get_random_virt_addr();

        for i in (0..NUM_TABLE_DESC_ENTRIES) {
            mapping_scheme_test_using_vaddr(vaddr + i * ONE_GIB);
        }

        for i in (0..NUM_TABLE_DESC_ENTRIES) {
            mapping_scheme_test_using_vaddr(vaddr + i * TWO_MIB);
        }

        for i in (0..NUM_TABLE_DESC_ENTRIES) {
            mapping_scheme_test_using_vaddr(vaddr + i * FOUR_KIB);
        }
    }

    #[test]
    fn insert_sanity_test() {
        let vaddr = get_random_virt_addr();

        insert_test_using_vaddr(vaddr + 1 * ONE_GIB);
        insert_test_using_vaddr(vaddr + 2 * TWO_MIB);
        insert_test_using_vaddr(vaddr + 3 * FOUR_KIB);
    }

    #[test]
    #[ignore]
    fn insert_long_test() {
        let vaddr = get_random_virt_addr();

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            insert_test_using_vaddr(vaddr + i * ONE_GIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            insert_test_using_vaddr(vaddr + i * TWO_MIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            insert_test_using_vaddr(vaddr + i * FOUR_KIB);
        });
    }
}
