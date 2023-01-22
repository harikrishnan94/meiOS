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

use core::{
    alloc::Layout,
    cell::UnsafeCell,
    cmp::{max, min},
    mem::size_of,
    ops::Range,
    ptr::NonNull,
};

use heapless::Vec;

use tock_registers::{
    interfaces::{ReadWriteable, Readable},
    registers::InMemoryRegister,
};

use crate::{
    address::{Address, AddressTranslationLevel, PhysicalAddress, VirtualAddress},
    bug,
    error::{Error, Result},
    mmu::NEXT_LEVEL_TABLE_ADDR_SHIFT,
    vm::{AccessPermissions, MapDesc, MemoryKind, MemoryMap, PhysicalPageAllocator},
};

use super::{
    utils::{consts::MAX_TRANSLATION_LEVELS, *},
    GRANULE_SIZE, LEVEL_1_OUTPUT_ADDR_SHIFT, LEVEL_2_OUTPUT_ADDR_SHIFT, LEVEL_3_OUTPUT_ADDR_SHIFT,
    STAGE1_BLOCK_DESCRIPTOR, STAGE1_LAST_LEVEL_DESCRIPTOR, STAGE1_PAGE_DESCRIPTOR,
    STAGE1_TABLE_DESCRIPTOR, TRANSLATION_TABLE_DESC_ALIGN,
};

const NUM_TABLE_DESC_ENTRIES: usize = 512;
const INVALID_DESCRIPTOR: u64 = 0;
const TRANSLATION_LEVELS: &[AddressTranslationLevel] = &[
    AddressTranslationLevel::Zero,
    AddressTranslationLevel::One,
    AddressTranslationLevel::Two,
    AddressTranslationLevel::Three,
];
const ONE_GIB: usize = consts::ONE_GIB;
const TWO_MIB: usize = consts::TWO_MIB;
const FOUR_KIB: usize = consts::FOUR_KIB;

type Stage1LastLevelDescriptor = InMemoryRegister<u64, STAGE1_LAST_LEVEL_DESCRIPTOR::Register>;
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

    /// Traverse a range of Virtual Address.
    /// For each mapping within the provided range, call the Visitor.
    pub fn traverse<'tt>(
        &'tt self,
        vaddr_rng: Range<VirtualAddress>,
        free_empty_descs: bool,
    ) -> impl Iterator<Item = Result<TraverseYield<'tt>>> {
        TraverseIterator::new(&self.root, vaddr_rng, free_empty_descs)
    }

    /// Walk the translation table using the VirtualAddress `vaddr` and produce corresponding PhysicalAddress
    /// This is similar to what CPU does after a TLB Miss.
    pub fn virt2phy(&self, vaddr: VirtualAddress) -> Option<TranslationDesc> {
        let mut descs = &self.root;

        for level in TRANSLATION_LEVELS.iter() {
            let idx = vaddr.get_idx_for_level(level);
            let desc = load_desc(descs, idx);

            let to_translation_desc = |desc: u64| {
                let ll_desc = Stage1LastLevelDescriptor::new(desc);
                let is_cacheable =
                    !ll_desc.matches_all(STAGE1_LAST_LEVEL_DESCRIPTOR::SH::OuterShareable);

                Some(TranslationDesc {
                    virt_addr: vaddr,
                    phy_addr: parse_output_address(&ll_desc, level),
                    access_perms: parse_access_perms(&ll_desc),
                    memory_kind: if is_cacheable {
                        MemoryKind::Normal
                    } else {
                        MemoryKind::Device
                    },
                })
            };

            match parse_desc(desc, level).ok()? {
                Descriptor::Table(tbl_desc) => {
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(block_desc) => return to_translation_desc(block_desc.get()),
                Descriptor::Page(page_desc) => return to_translation_desc(page_desc.get()),
                Descriptor::Invalid => return None,
            }
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
            find_best_mapping_scheme(map.virt_addr, map.phy_addr, map.num_pages * GRANULE_SIZE);
        let mut map = ParsedMemoryMap {
            phy_addr: map.phy_addr,
            virt_addr: map.virt_addr,
            attributes: map.attributes,
            num_pages: 0,
        };

        for scheme in map_scheme.spans {
            match scheme {
                ContiguousSpan::FourKiB(num_pages) => {
                    map.num_pages = num_pages;
                    while map.num_pages > 0 {
                        self.install_page_descs(&mut map, desc_alloc, mmap)
                            .map_err(|e| e)?;
                    }
                }
                ContiguousSpan::TwoMiB(num_pages) => {
                    map.num_pages = num_pages;
                    while map.num_pages > 0 {
                        self.install_l2_block_desc(&mut map, desc_alloc, mmap)
                            .map_err(|e| e)?;
                    }
                }
                ContiguousSpan::OneGiB(num_pages) => {
                    map.num_pages = num_pages;
                    while map.num_pages > 0 {
                        self.install_l1_block_desc(&mut map, desc_alloc, mmap)
                            .map_err(|e| e)?;
                    }
                }
            }
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

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) | Descriptor::Page(_) => {
                    return Err(Error::VMMapExists(*mmap))
                }

                Descriptor::Invalid => {
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

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) => return Err(Error::VMMapExists(*mmap)),
                Descriptor::Page(_) => return Err(Error::CorruptedTranslationTable(desc)),

                Descriptor::Invalid => {
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

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    descend_tbl_desc(tbl_desc, &mut descs);
                }
                Descriptor::Block(_) => {
                    if *level == AddressTranslationLevel::One {
                        return Err(Error::VMMapExists(*mmap));
                    } else {
                        return Err(Error::CorruptedTranslationTable(desc));
                    }
                }
                Descriptor::Page(_) => return Err(Error::CorruptedTranslationTable(desc)),
                Descriptor::Invalid => {
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
        }

        Ok(())
    }
}

pub enum TraverseYield<'tt> {
    PhysicalBlock(PhysicalBlockOverlapInfo<'tt>),
    UnusedMemory(NonNull<u8>),
}

/// Information about a physical block.
pub struct PhysicalBlockOverlapInfo<'tt> {
    /// Range of physical block, that contains the overlap.
    phy_block: PhysicalAddress,
    vaddr: VirtualAddress,
    size: u32,

    /// Offest within the above `phy_block`, which ovelaps the provided VA space.
    overlap: Range<u32>,
    desc_ptr: &'tt mut u64,
}

impl<'tt> PhysicalBlockOverlapInfo<'tt> {
    fn new(
        iter: &TraverseIterator<'tt>,
        paddr: PhysicalAddress,
        vaddr: VirtualAddress,
        block_size: u32,
        desc_ptr: &'tt mut u64,
    ) -> Self {
        let phy_start = paddr;
        let vaddr_start = vaddr;
        let vaddr_end = vaddr_start + block_size as usize;
        let va_space_overlap_start = max(vaddr_start, iter.va_rng.start);
        let va_space_overlap_end = min(vaddr_end, iter.va_rng.end);

        Self {
            phy_block: phy_start,
            vaddr: vaddr_start,
            size: block_size as u32,
            overlap: (va_space_overlap_start - vaddr_start) as u32
                ..(va_space_overlap_end - vaddr_start) as u32,
            desc_ptr,
        }
    }

    pub fn phy_block(&self) -> Range<PhysicalAddress> {
        self.phy_block..self.phy_block + self.size as usize
    }

    pub fn vaddr(&self) -> VirtualAddress {
        self.vaddr
    }

    pub fn size(&self) -> usize {
        self.size as usize
    }

    pub fn overlapping_range(&self) -> &Range<u32> {
        &self.overlap
    }

    pub fn non_overlapping_range(&self) -> (Range<u32>, Range<u32>) {
        (0..self.overlap.start, self.overlap.end..self.size)
    }

    pub fn remove_overlapping_range<DescAlloc: PhysicalPageAllocator>(
        &mut self,
        tt: &TranslationTable,
        desc_alloc: &DescAlloc,
    ) -> Result<()> {
        let ll_desc = Stage1LastLevelDescriptor::new(*self.desc_ptr);

        // Remove the existing mapping
        *self.desc_ptr = INVALID_DESCRIPTOR;

        // Physical Block:
        // [..........MMMM...........]
        // [A         B  C          D]
        // first_range = [A, B)
        // overlapping_range = [B, C)
        // last_range = [C, D)
        let (first_rng, last_rng) = self.non_overlapping_range();

        // Install the new ranges (Non-Overlapping)
        if let Some(map) = self.create_memory_map(first_rng, &ll_desc) {
            tt.map(&map, desc_alloc)?;
        }
        if let Some(map) = self.create_memory_map(last_rng, &ll_desc) {
            tt.map(&map, desc_alloc)?;
        }

        Ok(())
    }

    fn create_memory_map(
        &self,
        rng: Range<u32>,
        ll_desc: &Stage1LastLevelDescriptor,
    ) -> Option<MemoryMap> {
        let num_pages = (rng.end - rng.start) as usize / GRANULE_SIZE;
        if num_pages == 0 {
            return None;
        }

        let access_perms = parse_access_perms(ll_desc);
        let paddr = self.phy_block + rng.start as usize;
        let vaddr = self.vaddr + rng.start as usize;
        let is_cacheable = !ll_desc.matches_all(STAGE1_LAST_LEVEL_DESCRIPTOR::SH::OuterShareable);
        let map = MapDesc::new(paddr, vaddr, num_pages, access_perms);

        Some(match is_cacheable {
            true => MemoryMap::Normal(map),
            false => MemoryMap::Device(map),
        })
    }
}

type Stash<'tt> = Vec<&'tt DescriptorTable, MAX_TRANSLATION_LEVELS>;

#[derive(Clone, Copy)]
enum IterState {
    Load,
    Step,
    MoveRight,
    Halt,
}

pub struct TraverseIterator<'tt> {
    root: &'tt DescriptorTable,
    va_rng: Range<VirtualAddress>,
    va_space_explored: VirtualAddress,

    empty_descs: Vec<NonNull<u8>, MAX_TRANSLATION_LEVELS>,
    stash: Stash<'tt>,

    should_free_empty_descs: bool,
    state: Result<IterState>,
}

impl<'tt> Iterator for TraverseIterator<'tt> {
    type Item = Result<TraverseYield<'tt>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(empty_desc) = self.empty_descs.pop() {
            return Some(Ok(TraverseYield::UnusedMemory(empty_desc)));
        }

        match self.fetch_block() {
            FetchBlockResult::Block(pbo_info) => Some(Ok(TraverseYield::PhysicalBlock(pbo_info))),
            FetchBlockResult::Err(e) => Some(Err(e)),
            FetchBlockResult::None => None,
        }
    }
}

enum FetchBlockResult<'tt> {
    Block(PhysicalBlockOverlapInfo<'tt>),
    Err(Error),
    None,
}

impl<'tt> TraverseIterator<'tt> {
    fn new(
        root: &'tt DescriptorTable,
        mut va_rng: Range<VirtualAddress>,
        should_free_empty_descs: bool,
    ) -> Self {
        // Align start and end to page boundary.
        va_rng.start.align_down(FOUR_KIB);
        va_rng.end.align_up(FOUR_KIB);

        let mut iter = TraverseIterator {
            root,
            va_rng: va_rng.clone(),
            should_free_empty_descs,
            va_space_explored: VirtualAddress::new(0).unwrap(),
            empty_descs: Vec::default(),
            stash: Vec::default(),
            state: Ok(IterState::Halt),
        };

        iter.begin();
        iter
    }

    fn begin(&mut self) {
        if self.va_rng.start >= self.va_rng.end {
            return;
        }

        self.state = (|| -> Result<IterState> {
            let mut descs = self.root;
            let vaddr = self.va_rng.start;

            for level in TRANSLATION_LEVELS.iter() {
                let idx = vaddr.get_idx_for_level(level);
                let desc = load_desc(descs, idx);

                self.va_space_explored.set_idx_for_level(level, idx);
                self.stash
                    .push(descs)
                    .unwrap_or_else(|_| bug!("stash size exceeded"));

                match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                    Descriptor::Table(child_descs) => {
                        descs = TraverseIterator::descend(&child_descs, level, vaddr).0;
                    }
                    _ => break,
                }
            }
            Ok(IterState::Load)
        })();

        assert!(self.va_space_explored < self.va_rng.end);
    }

    fn fetch_block(&mut self) -> FetchBlockResult<'tt> {
        while !self.stash.is_empty() {
            match self.state {
                Ok(state) => match state {
                    IterState::Load => {
                        if let Some(pbo_info) = self.load_block() {
                            self.state = Ok(IterState::MoveRight);
                            return FetchBlockResult::Block(pbo_info);
                        } else {
                            self.state = Ok(IterState::Step);
                        }
                    }
                    IterState::MoveRight => self.state = Ok(self.move_right()),
                    IterState::Step => self.state = self.step(),
                    IterState::Halt => break,
                },
                Err(e) => return FetchBlockResult::Err(e),
            }
        }

        FetchBlockResult::None
    }

    fn step(&mut self) -> Result<IterState> {
        assert!(!self.stash.is_empty());

        let descs = self
            .stash
            .last()
            .unwrap_or_else(|| bug!("bug on load_block"))
            .clone();
        let level = AddressTranslationLevel::from(self.stash.len() - 1);
        let idx = self.va_space_explored.get_idx_for_level(&level);

        assert!(idx < NUM_TABLE_DESC_ENTRIES);
        assert!(self.va_space_explored < self.va_rng.end);

        let desc = load_desc(descs, idx as usize);
        match parse_desc(desc, &level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
            Descriptor::Table(child_descs) => {
                let child =
                    TraverseIterator::descend(&child_descs, &level, self.va_space_explored).0;
                self.stash
                    .push(child)
                    .unwrap_or_else(|_| bug!("stash size exceeded"));
                return Ok(IterState::Load);
            }
            _ => return Ok(self.move_right()),
        }
    }

    fn move_right(&mut self) -> IterState {
        assert!(!self.stash.is_empty());

        let descs = self
            .stash
            .last()
            .unwrap_or_else(|| bug!("bug on load_block"))
            .clone();
        let level = AddressTranslationLevel::from(self.stash.len() - 1);
        let idx = self.va_space_explored.get_idx_for_level(&level);

        assert!(idx < NUM_TABLE_DESC_ENTRIES);
        assert!(self.va_space_explored < self.va_rng.end);

        if self.find_next_valid_entry(descs, &level, idx + 1) {
            return IterState::Load;
        } else {
            return self.move_up(descs, level);
        }
    }

    fn move_up(&mut self, descs: &DescriptorTable, level: AddressTranslationLevel) -> IterState {
        assert!(!self.stash.is_empty());

        if self.va_space_explored >= self.va_rng.end {
            while self.ascend() {
                // Allow Empty descriptors to be visited and free'd
            }
            return IterState::Halt;
        }

        // Move up until, a valid entry can be found.
        while self.ascend() {
            let parent = self
                .stash
                .last()
                .unwrap_or_else(|| bug!("bug on load_block"))
                .clone();
            let parent_level = AddressTranslationLevel::from(self.stash.len() - 1);
            let parent_idx = self.va_space_explored.get_idx_for_level(&parent_level);

            if self.find_next_valid_entry(parent, &parent_level, parent_idx + 1) {
                return IterState::Load;
            }
        }
        IterState::Halt
    }

    fn ascend(&mut self) -> bool {
        let level = AddressTranslationLevel::from(self.stash.len() - 1);
        let descs = self.stash.pop().unwrap_or_else(|| bug!("bug in ascend"));

        self.free_descs_if_empty(descs, &level);
        self.va_space_explored.clear_idx_for_level(&level);
        !self.stash.is_empty()
    }

    fn descend(
        descs: &Stage1TableDescriptor,
        level: &AddressTranslationLevel,
        vaddr: VirtualAddress,
    ) -> (&'tt DescriptorTable, usize) {
        let child = get_next_level_desc(descs);
        let child_level = level.next();
        let child_idx = vaddr.get_idx_for_level(&child_level);

        (child, child_idx)
    }

    fn load_block(&self) -> Option<PhysicalBlockOverlapInfo<'tt>> {
        let descs = self
            .stash
            .last()
            .unwrap_or_else(|| bug!("bug on load_block"))
            .clone();
        let level = AddressTranslationLevel::from(self.stash.len() - 1);
        let idx = self.va_space_explored.get_idx_for_level(&level);

        let desc = load_desc(descs, idx as usize);
        match parse_desc(desc, &level)
            .map_err(|_| Error::CorruptedTranslationTable(desc))
            .unwrap_or_else(|_| bug!("parsing verified descriptor failed"))
        {
            Descriptor::Block(_) | Descriptor::Page(_) => {
                Some(self.create_phy_block_info(descs, &level, idx))
            }
            _ => None,
        }
    }

    fn find_next_valid_entry(
        &mut self,
        descs: &DescriptorTable,
        level: &AddressTranslationLevel,
        mut idx: usize,
    ) -> bool {
        loop {
            if idx < NUM_TABLE_DESC_ENTRIES {
                self.va_space_explored.set_idx_for_level(&level, idx);

                if self.va_space_explored < self.va_rng.end {
                    let desc = load_desc(descs, idx as usize);

                    if matches!(to_raw_desc(desc), RawDescriptor::Invalid) {
                        idx += 1;
                    } else {
                        return true;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        false
    }

    fn free_descs_if_empty(&mut self, descs: &DescriptorTable, level: &AddressTranslationLevel) {
        if !self.should_free_empty_descs || level == &AddressTranslationLevel::Zero {
            return;
        }

        for idx in 0..NUM_TABLE_DESC_ENTRIES {
            let desc = load_desc(descs, idx);
            let desc = Stage1LastLevelDescriptor::new(desc);

            if desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::VALID) {
                return;
            }
        }

        let parent_level = level.prev();
        let parent = self.stash[parent_level as usize];
        let parent_idx = self.va_space_explored.get_idx_for_level(&parent_level);

        *load_desc_mut(parent, parent_idx) = INVALID_DESCRIPTOR;
        self.empty_descs
            .push(NonNull::from(unsafe { &*(descs.0.get() as *const u8) }))
            .unwrap_or_else(|_| bug!("empty_descs size exceeded"));
    }

    fn create_phy_block_info(
        &self,
        descs: &'tt DescriptorTable,
        level: &AddressTranslationLevel,
        idx: usize,
    ) -> PhysicalBlockOverlapInfo<'tt> {
        assert!(self.va_space_explored < self.va_rng.end);

        let desc = load_desc_mut(descs, idx as usize);
        let ll_desc = Stage1LastLevelDescriptor::new(*desc);

        let paddr = match level {
            AddressTranslationLevel::Zero => bug!("invalid level to fetch output address"),
            AddressTranslationLevel::One => {
                ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_1GiB) as usize * ONE_GIB
            }
            AddressTranslationLevel::Two => {
                ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_2MiB) as usize * TWO_MIB
            }
            AddressTranslationLevel::Three => {
                ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_4KiB) as usize * FOUR_KIB
            }
        };

        PhysicalBlockOverlapInfo::new(
            self,
            PhysicalAddress::new(paddr),
            self.va_space_explored,
            get_vaddr_spacing_per_entry(level) as u32,
            desc,
        )
    }
}

fn get_next_level_desc<'tt>(tbl_desc: &Stage1TableDescriptor) -> &'tt DescriptorTable {
    let next_lvl_desc = read_next_level_desc(&tbl_desc);
    assert_ne!(next_lvl_desc, 0);
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
}

#[derive(Debug, Clone, Copy)]
enum ContiguousSpan {
    /// Number of Pages in 4KiB boundary
    FourKiB(usize),
    /// Number of Pages in 2MiB boundary
    TwoMiB(usize),
    /// Number of Pages in 1GiB boundary
    OneGiB(usize),
}

const ALIGNMENTS: [usize; 3] = [ONE_GIB, TWO_MIB, FOUR_KIB];
const MAX_MAPPING_SPANS: usize = max_mapping_spans(ALIGNMENTS.len());

#[derive(Default)]
struct MappingScheme {
    spans: Vec<ContiguousSpan, MAX_MAPPING_SPANS>,
}

fn find_best_mapping_scheme_impl(
    scheme: &mut MappingScheme,
    vaddr: VirtualAddress,
    paddr: PhysicalAddress,
    size: usize,
    level: usize,
) {
    if size == 0 {
        return;
    }

    fn align_offset<A: Address>(a: A, align: usize) -> usize {
        let offset = a.align_offset(align);
        if offset != 0 {
            align - offset
        } else {
            0
        }
    }

    let align = ALIGNMENTS[level];
    let va_offset = align_offset(vaddr, align);
    let pa_offset = align_offset(paddr, align);

    if va_offset == pa_offset && size > pa_offset && size - pa_offset >= align {
        let current_span_paddr = PhysicalAddress::new(paddr.align_up(align));
        let current_span_size = {
            let span_end = current_span_paddr + size - pa_offset;
            span_end.align_down(align);
            (span_end - current_span_paddr) as usize
        };
        assert_ne!(current_span_size, 0);

        find_best_mapping_scheme_impl(scheme, vaddr, paddr, pa_offset, level + 1);

        let page_count = current_span_size / align;
        if page_count != 0 {
            scheme
                .spans
                .push(match align {
                    ONE_GIB => ContiguousSpan::OneGiB(page_count),
                    TWO_MIB => ContiguousSpan::TwoMiB(page_count),
                    FOUR_KIB => ContiguousSpan::FourKiB(page_count),
                    _ => bug!("Cannot reach here"),
                })
                .unwrap_or_else(|_| bug!("spans limit reached"));
        }

        find_best_mapping_scheme_impl(
            scheme,
            VirtualAddress::new(vaddr.align_up(align))
                .unwrap_or_else(|_| bug!("bug in constructing aligned virtual address"))
                + current_span_size,
            current_span_paddr + current_span_size,
            size - current_span_size - pa_offset,
            level + 1,
        );
    } else {
        find_best_mapping_scheme_impl(scheme, vaddr, paddr, size, level + 1);
    }
}

fn find_best_mapping_scheme(
    vaddr: VirtualAddress,
    paddr: PhysicalAddress,
    size: usize,
) -> MappingScheme {
    assert!(vaddr.is_aligned(GRANULE_SIZE));
    assert!(paddr.is_aligned(GRANULE_SIZE));

    let mut scheme = MappingScheme::default();

    find_best_mapping_scheme_impl(&mut scheme, vaddr, paddr, size, 0);
    scheme
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

fn parse_output_address(
    ll_desc: &Stage1LastLevelDescriptor,
    level: &AddressTranslationLevel,
) -> PhysicalAddress {
    match level {
        AddressTranslationLevel::Zero => bug!("unexpected level for parse_output_address"),
        AddressTranslationLevel::One => {
            assert!(!ll_desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::TYPE));
            PhysicalAddress::new(
                (ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_1GiB)
                    << LEVEL_1_OUTPUT_ADDR_SHIFT) as usize,
            )
        }
        AddressTranslationLevel::Two => {
            assert!(!ll_desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::TYPE));
            PhysicalAddress::new(
                (ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_2MiB)
                    << LEVEL_2_OUTPUT_ADDR_SHIFT) as usize,
            )
        }
        AddressTranslationLevel::Three => {
            assert!(ll_desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::TYPE));
            PhysicalAddress::new(
                (ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::OUTPUT_ADDR_4KiB)
                    << LEVEL_3_OUTPUT_ADDR_SHIFT) as usize,
            )
        }
    }
}

fn parse_access_perms(ll_desc: &Stage1LastLevelDescriptor) -> AccessPermissions {
    let ap = ll_desc.read(STAGE1_LAST_LEVEL_DESCRIPTOR::AP);

    let mut access_perms = if ap == STAGE1_LAST_LEVEL_DESCRIPTOR::AP::RW_EL1_EL0.value {
        AccessPermissions::EL1_READ
            | AccessPermissions::EL1_WRITE
            | AccessPermissions::EL0_READ
            | AccessPermissions::EL0_WRITE
    } else if ap == STAGE1_LAST_LEVEL_DESCRIPTOR::AP::RW_EL1.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE
    } else if ap == STAGE1_LAST_LEVEL_DESCRIPTOR::AP::RO_EL1_EL0.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL0_READ
    } else if ap == STAGE1_LAST_LEVEL_DESCRIPTOR::AP::RO_EL1.value {
        AccessPermissions::EL1_READ
    } else {
        bug!("Invalid Access Permissions on page");
    };

    if !ll_desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::PXN)
        && !access_perms.contains(AccessPermissions::EL1_WRITE)
    {
        access_perms |= AccessPermissions::EL1_EXECUTE;
    }
    if !ll_desc.is_set(STAGE1_LAST_LEVEL_DESCRIPTOR::UXN)
        && !access_perms.contains(AccessPermissions::EL0_WRITE)
    {
        access_perms |= AccessPermissions::EL0_EXECUTE;
    }

    access_perms
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

const fn max_mapping_spans(num_aligns: usize) -> usize {
    if num_aligns == 0 {
        0
    } else {
        1 + 2 * max_mapping_spans(num_aligns - 1)
    }
}

fn load_desc(descs: &DescriptorTable, idx: usize) -> u64 {
    unsafe { (*descs.0.get())[idx] }
}

fn load_desc_mut(descs: &DescriptorTable, idx: usize) -> &mut u64 {
    unsafe { &mut (*descs.0.get())[idx] }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::{
        alloc::{AllocError, Allocator, Layout},
        cell::RefCell,
        mem::size_of,
        ptr::NonNull,
    };
    use rand::{
        distributions::{Distribution, Uniform},
        seq::SliceRandom,
        thread_rng, Rng,
    };
    use rayon::prelude::*;
    use std::{collections::HashMap, vec, vec::Vec};

    use crate::{
        address::{PhysicalAddress, VirtualAddress},
        bug,
        mmu::{
            translation_table::{
                ContiguousSpan, DescriptorTable, TranslationTable, TraverseYield,
                NUM_TABLE_DESC_ENTRIES,
            },
            GRANULE_SIZE, OUTPUT_ADDR_BITS, TRANSLATION_TABLE_DESC_ALIGN,
        },
        vm::{AccessPermissions, MapDesc, MemoryKind, MemoryMap, PhysicalPageAllocator},
    };

    use super::{find_best_mapping_scheme, FOUR_KIB, ONE_GIB, TWO_MIB};

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

    fn traverse_test_using_vaddr(vaddr: VirtualAddress) {
        let page_alloc = TestAllocator::default();
        let memory_maps = generate_memory_maps(vaddr);
        let layout =
            Layout::from_size_align(size_of::<DescriptorTable>(), TRANSLATION_TABLE_DESC_ALIGN)
                .unwrap_or_else(|_| bug!("Descriptor Layout Mismatch"));
        let translation_table = TranslationTable::new(&memory_maps, &page_alloc);

        assert!(translation_table.is_ok());

        let translation_table = translation_table.unwrap();

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let paddr = desc.physical_address();
                    let map_size = desc.num_pages() * FOUR_KIB;
                    let mut size = 0;

                    for res in translation_table.traverse(vaddr..vaddr + map_size, true) {
                        assert!(res.is_ok());

                        match res.unwrap() {
                            TraverseYield::PhysicalBlock(mut pbo_info) => {
                                assert_eq!(pbo_info.phy_block().start, paddr + size);
                                assert_eq!(pbo_info.vaddr(), vaddr + size);
                                let overlap = pbo_info.phy_block();
                                size += (overlap.end - overlap.start) as usize;

                                let remove = pbo_info
                                    .remove_overlapping_range(&translation_table, &page_alloc);
                                assert!(remove.is_ok());
                            }
                            TraverseYield::UnusedMemory(mem) => unsafe {
                                page_alloc.deallocate(mem, layout)
                            },
                        }
                    }

                    assert_eq!(size, map_size);
                }
                MemoryMap::Device(_) => assert!(false, "Failure"),
            }
        }
    }

    fn lookup_test_using_vaddr(vaddr: VirtualAddress) {
        let page_alloc = TestAllocator::default();
        let memory_maps = generate_memory_maps(vaddr);
        let layout =
            Layout::from_size_align(size_of::<DescriptorTable>(), TRANSLATION_TABLE_DESC_ALIGN)
                .unwrap_or_else(|_| bug!("Descriptor Layout Mismatch"));
        let translation_table = TranslationTable::new(&memory_maps, &page_alloc);

        assert!(translation_table.is_ok());

        let translation_table = translation_table.unwrap();
        let mut rng = thread_rng();

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let paddr = desc.physical_address();
                    let map_size = desc.num_pages() * FOUR_KIB;
                    let mut traversed_size = 0;
                    let unmap_start = Uniform::from(0..desc.num_pages()).sample(&mut rng);
                    let unmap_end =
                        Uniform::from(unmap_start + 1..=desc.num_pages()).sample(&mut rng);
                    let unmap_rng =
                        vaddr + unmap_start * GRANULE_SIZE..vaddr + unmap_end * GRANULE_SIZE;

                    for res in translation_table.traverse(unmap_rng.clone(), true) {
                        assert!(res.is_ok());

                        match res.unwrap() {
                            TraverseYield::PhysicalBlock(pbo_info) => {
                                let pblock = pbo_info.phy_block();
                                let overlap = pbo_info.overlapping_range();
                                assert_eq!(
                                    pblock.start + overlap.start as usize,
                                    paddr + unmap_start * GRANULE_SIZE + traversed_size
                                );
                                assert_eq!(
                                    pbo_info.vaddr() + overlap.start as usize,
                                    vaddr + unmap_start * GRANULE_SIZE + traversed_size
                                );
                                traversed_size += (overlap.end - overlap.start) as usize;
                            }
                            TraverseYield::UnusedMemory(mem) => unsafe {
                                page_alloc.deallocate(mem, layout)
                            },
                        }
                    }

                    assert_eq!(traversed_size, (unmap_rng.end - unmap_rng.start) as usize);
                }
                MemoryMap::Device(_) => assert!(false, "Failure"),
            }
        }
    }

    fn remove_test_using_vaddr(vaddr: VirtualAddress) {
        let page_alloc = TestAllocator::default();
        let memory_maps = generate_memory_maps(vaddr);
        let layout =
            Layout::from_size_align(size_of::<DescriptorTable>(), TRANSLATION_TABLE_DESC_ALIGN)
                .unwrap_or_else(|_| bug!("Descriptor Layout Mismatch"));
        let translation_table = TranslationTable::new(&memory_maps, &page_alloc);

        assert!(translation_table.is_ok());

        let translation_table = translation_table.unwrap();
        let mut rng = thread_rng();

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let unmap_start = Uniform::from(0..desc.num_pages()).sample(&mut rng);
                    let unmap_end =
                        Uniform::from(unmap_start + 1..=desc.num_pages()).sample(&mut rng);
                    let unmap_rng =
                        vaddr + unmap_start * GRANULE_SIZE..vaddr + unmap_end * GRANULE_SIZE;
                    let mut traversed_size = 0;

                    for res in translation_table.traverse(unmap_rng.clone(), true) {
                        assert!(res.is_ok());

                        match res.unwrap() {
                            TraverseYield::PhysicalBlock(mut pbo_info) => {
                                let overlap = pbo_info.overlapping_range();
                                traversed_size += (overlap.end - overlap.start) as usize;

                                let remove = pbo_info
                                    .remove_overlapping_range(&translation_table, &page_alloc);
                                assert!(remove.is_ok());
                            }
                            TraverseYield::UnusedMemory(mem) => unsafe {
                                page_alloc.deallocate(mem, layout)
                            },
                        }
                    }

                    assert_eq!(traversed_size, (unmap_rng.end - unmap_rng.start) as usize);

                    let count_after_removal =
                        translation_table.traverse(unmap_rng.clone(), true).count();
                    assert_eq!(count_after_removal, 0);
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
                    let scheme = find_best_mapping_scheme(vaddr, paddr, size);
                    let mut mapped_size = 0;

                    for scheme in scheme.spans {
                        mapped_size += match scheme {
                            ContiguousSpan::FourKiB(num_pages) => num_pages * FOUR_KIB,
                            ContiguousSpan::TwoMiB(num_pages) => num_pages * TWO_MIB,
                            ContiguousSpan::OneGiB(num_pages) => num_pages * ONE_GIB,
                        }
                    }

                    assert_eq!(mapped_size, size);
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
    fn traverse_sanity_test() {
        let vaddr = get_random_virt_addr();

        traverse_test_using_vaddr(vaddr + 1 * ONE_GIB);
        traverse_test_using_vaddr(vaddr + 2 * TWO_MIB);
        traverse_test_using_vaddr(vaddr + 3 * FOUR_KIB);
    }

    #[test]
    fn lookup_sanity_test() {
        let vaddr = get_random_virt_addr();

        lookup_test_using_vaddr(vaddr + 1 * ONE_GIB);
        lookup_test_using_vaddr(vaddr + 2 * TWO_MIB);
        lookup_test_using_vaddr(vaddr + 3 * FOUR_KIB);
    }

    #[test]
    fn remove_sanity_test() {
        // let vaddr = get_random_virt_addr();

        // remove_test_using_vaddr(vaddr + 1 * ONE_GIB);
        // remove_test_using_vaddr(vaddr + 2 * TWO_MIB);
        // remove_test_using_vaddr(vaddr + 3 * FOUR_KIB);

        let page_alloc = TestAllocator::default();
        let memory_maps = vec![MemoryMap::Normal(MapDesc::new(
            PhysicalAddress::new(32229031936),
            VirtualAddress::new(205722300186624).unwrap(),
            512,
            AccessPermissions::normal_memory_default(),
        ))];
        let layout =
            Layout::from_size_align(size_of::<DescriptorTable>(), TRANSLATION_TABLE_DESC_ALIGN)
                .unwrap_or_else(|_| bug!("Descriptor Layout Mismatch"));
        let translation_table = TranslationTable::new(&memory_maps, &page_alloc);

        assert!(translation_table.is_ok());

        let translation_table = translation_table.unwrap();
        let mut rng = thread_rng();

        for map in &memory_maps {
            match map {
                MemoryMap::Normal(desc) => {
                    let vaddr = desc.virtual_address();
                    let unmap_start = Uniform::from(0..desc.num_pages()).sample(&mut rng);
                    let unmap_end =
                        Uniform::from(unmap_start + 1..=desc.num_pages()).sample(&mut rng);
                    let unmap_rng =
                        vaddr + unmap_start * GRANULE_SIZE..vaddr + unmap_end * GRANULE_SIZE;
                    let mut traversed_size = 0;

                    for res in translation_table.traverse(unmap_rng.clone(), true) {
                        assert!(res.is_ok());

                        match res.unwrap() {
                            TraverseYield::PhysicalBlock(mut pbo_info) => {
                                let overlap = pbo_info.overlapping_range();
                                traversed_size += (overlap.end - overlap.start) as usize;

                                let remove = pbo_info
                                    .remove_overlapping_range(&translation_table, &page_alloc);
                                assert!(remove.is_ok());
                            }
                            TraverseYield::UnusedMemory(mem) => unsafe {
                                page_alloc.deallocate(mem, layout)
                            },
                        }
                    }

                    assert_eq!(traversed_size, (unmap_rng.end - unmap_rng.start) as usize);

                    let count_after_removal =
                        translation_table.traverse(unmap_rng.clone(), true).count();
                    assert_eq!(count_after_removal, 0);
                }
                MemoryMap::Device(_) => assert!(false, "Failure"),
            }
        }
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

    #[test]
    #[ignore]
    fn traverse_long_test() {
        let vaddr = get_random_virt_addr();

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            traverse_test_using_vaddr(vaddr + i * ONE_GIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            traverse_test_using_vaddr(vaddr + i * TWO_MIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            traverse_test_using_vaddr(vaddr + i * FOUR_KIB);
        });
    }

    #[test]
    #[ignore]
    fn lookup_long_test() {
        let vaddr = get_random_virt_addr();

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            lookup_test_using_vaddr(vaddr + i * ONE_GIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            lookup_test_using_vaddr(vaddr + i * TWO_MIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            lookup_test_using_vaddr(vaddr + i * FOUR_KIB);
        });
    }

    #[test]
    #[ignore]
    fn remove_long_test() {
        let vaddr = get_random_virt_addr();

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            remove_test_using_vaddr(vaddr + i * ONE_GIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            remove_test_using_vaddr(vaddr + i * TWO_MIB);
        });

        (0..NUM_TABLE_DESC_ENTRIES).into_par_iter().for_each(|i| {
            remove_test_using_vaddr(vaddr + i * FOUR_KIB);
        });
    }
}
