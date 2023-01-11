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

use core::cell::RefCell;

use tock_registers::{
    interfaces::{ReadWriteable, Readable},
    registers::InMemoryRegister,
};

use crate::{
    address::{Address, AddressTranslationLevel, PhysicalAddress, VirtualAddress},
    bug,
    error::{Error, Result},
    mmu::TRANSLATION_TABLE_DESC_ALIGN_BITS,
    vm::{
        physical_page_alloc::{AllocationLayout, Allocator},
        AccessPermissions, MemoryKind, MemoryMap,
    },
};

use super::{
    LEVEL_1_OUTPUT_ADDR_SHIFT, LEVEL_2_OUTPUT_ADDR_SHIFT, LEVEL_3_OUTPUT_ADDR_SHIFT,
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

type Stage1PageDescriptor = InMemoryRegister<u64, STAGE1_PAGE_DESCRIPTOR::Register>;
type Stage1TableDescriptor = InMemoryRegister<u64, STAGE1_TABLE_DESCRIPTOR::Register>;
type Stage1BlockDescriptor = InMemoryRegister<u64, STAGE1_BLOCK_DESCRIPTOR::Register>;

/// Translation Table Descriptors
#[derive(Debug)]
#[repr(C)]
#[repr(align(4096))]
struct DescriptorTable([u64; NUM_TABLE_DESC_ENTRIES]);

impl Default for DescriptorTable {
    fn default() -> Self {
        Self([INVALID_DESCRIPTOR; NUM_TABLE_DESC_ENTRIES])
    }
}

/// This stores the root of Translation Table
/// Address of `root` is stored in TTBR0/1
pub struct TranslationTable {
    root: DescriptorTable,
}

impl TranslationTable {
    pub fn new<PhyAllocator: Allocator>(
        maps: &[MemoryMap],
        phy_page_alloc: &mut PhyAllocator,
    ) -> Result<Self> {
        let mut tt = Self {
            root: DescriptorTable::default(),
        };

        for map in maps {
            tt.add_impl(&parse_memory_map(map), phy_page_alloc, map)?;
        }

        Ok(tt)
    }

    /// Add Mapping to translation table
    pub fn add<PhyAllocator: Allocator>(
        &mut self,
        map: &MemoryMap,
        phy_page_alloc: &mut PhyAllocator,
    ) -> Result<()> {
        self.add_impl(&parse_memory_map(map), phy_page_alloc, map)
    }

    /// Walk the translation table using the VirtualAddress `vaddr` and produce corresponding PhysicalAddress
    /// This is similar to what CPU does after a TLB Miss.
    pub fn virt2phy(&self, vaddr: VirtualAddress) -> Option<TranslationDesc> {
        let mut visitor = ROVisitor::new(vaddr);
        // Casting is OK, here as long as Visitor doesn't change the table.
        unsafe { &mut *(self as *const Self as usize as *mut Self) }
            .walk_generic(vaddr, &mut visitor)
            .ok()?;

        visitor.res
    }

    pub fn get_base_address(&self) -> u64 {
        self.root.0.as_ptr() as u64
    }

    fn add_impl<PhyAllocator: Allocator>(
        &mut self,
        map: &ParsedMemoryMap,
        phy_page_alloc: &mut PhyAllocator,
        mmap: &MemoryMap,
    ) -> Result<()> {
        let mut visitor = InsertVisitor::new(map, phy_page_alloc, mmap);
        self.walk_generic(map.virt_addr, &mut visitor)?;
        Ok(())
    }

    fn walk_generic<V: Visitor>(&mut self, vaddr: VirtualAddress, visitor: &mut V) -> Result<()> {
        let descs = RefCell::new(&mut self.root);

        let visit_tbl_desc = |tbl_desc: &Stage1TableDescriptor| {
            let next_lvl_desc = tbl_desc.read(STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR)
                << TRANSLATION_TABLE_DESC_ALIGN_BITS;

            *descs.borrow_mut() = unsafe { &mut *(next_lvl_desc as *mut DescriptorTable) };
        };

        for level in TRANSLATION_LEVELS {
            let idx = vaddr.get_idx_for_level(level);
            let desc = descs.borrow().0[idx];

            match parse_desc(desc, level).map_err(|_| Error::CorruptedTranslationTable(desc))? {
                Descriptor::Table(tbl_desc) => {
                    assert_ne!(level, &AddressTranslationLevel::Three);
                    visit_tbl_desc(&tbl_desc);
                }
                Descriptor::Block(block_desc) => {
                    assert!(
                        level == &AddressTranslationLevel::One
                            || level == &AddressTranslationLevel::Two
                    );
                    if level == &AddressTranslationLevel::One {
                        match visitor.visit_block_desc_lvl1(&block_desc)? {
                            TraversalControl::Break => break,
                            TraversalControl::Continue => continue,
                            TraversalControl::UpdateAndContinue(tbl_desc) => {
                                descs.borrow_mut().0[idx] = tbl_desc.get();
                                visit_tbl_desc(&tbl_desc);
                                continue;
                            }
                            TraversalControl::UpdateAndBreak(newval) => {
                                descs.borrow_mut().0[idx] = newval;
                                break;
                            }
                        }
                    } else {
                        match visitor.visit_block_desc_lvl2(&block_desc)? {
                            TraversalControl::Break => break,
                            TraversalControl::Continue => continue,
                            TraversalControl::UpdateAndContinue(tbl_desc) => {
                                descs.borrow_mut().0[idx] = tbl_desc.get();
                                visit_tbl_desc(&tbl_desc);
                                continue;
                            }
                            TraversalControl::UpdateAndBreak(newval) => {
                                descs.borrow_mut().0[idx] = newval;
                                break;
                            }
                        }
                    }
                }
                Descriptor::Page(page_desc) => {
                    assert_eq!(level, &AddressTranslationLevel::Three);
                    match visitor.visit_page_desc(&page_desc)? {
                        TraversalControl::Break | TraversalControl::Continue => break,
                        TraversalControl::UpdateAndContinue(_) => {}
                        TraversalControl::UpdateAndBreak(new_desc) => match to_raw_desc(new_desc) {
                            RawDescriptor::TableOrPage(new_desc) => {
                                descs.borrow_mut().0[idx] = new_desc;
                                break;
                            }
                            RawDescriptor::Block(_) => {}
                            RawDescriptor::Invalid => {
                                descs.borrow_mut().0[idx] = new_desc;
                                break;
                            }
                        },
                    }
                }
                Descriptor::Invalid => match visitor.visit_invalid(desc, level)? {
                    TraversalControl::Break | TraversalControl::Continue => break,
                    TraversalControl::UpdateAndContinue(tbl_desc) => {
                        descs.borrow_mut().0[idx] = tbl_desc.get();
                        visit_tbl_desc(&tbl_desc);
                        continue;
                    }
                    TraversalControl::UpdateAndBreak(newval) => {
                        descs.borrow_mut().0[idx] = newval;
                        break;
                    }
                },
            }
        }

        unreachable!("Cannot Reach here after 4 levels");
    }
}

trait Visitor {
    fn visit_page_desc(&mut self, desc: &Stage1PageDescriptor) -> Result<TraversalControl>;

    fn visit_block_desc_lvl1(&mut self, desc: &Stage1BlockDescriptor) -> Result<TraversalControl>;

    fn visit_block_desc_lvl2(&mut self, desc: &Stage1BlockDescriptor) -> Result<TraversalControl>;

    fn visit_invalid(
        &mut self,
        desc_val: u64,
        level: &AddressTranslationLevel,
    ) -> Result<TraversalControl>;
}

enum TraversalControl {
    Break,
    Continue,
    UpdateAndContinue(Stage1TableDescriptor),
    UpdateAndBreak(u64),
}

struct ROVisitor {
    vaddr: VirtualAddress,
    res: Option<TranslationDesc>,
}

impl ROVisitor {
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr, res: None }
    }

    fn block_desc_complete_visit(
        &mut self,
        block_desc: &Stage1BlockDescriptor,
        phy_addr: usize,
    ) -> TraversalControl {
        let is_cacheable = !block_desc.matches_all(STAGE1_BLOCK_DESCRIPTOR::SH::OuterShareable);

        self.res = Some(TranslationDesc {
            virt_addr: self.vaddr,
            phy_addr: PhysicalAddress::new(phy_addr),
            access_perms: parse_access_perms_bd(block_desc),
            memory_kind: if is_cacheable {
                MemoryKind::Normal
            } else {
                MemoryKind::Device
            },
        });

        TraversalControl::Break
    }
}

impl Visitor for ROVisitor {
    fn visit_page_desc(&mut self, page_desc: &Stage1PageDescriptor) -> Result<TraversalControl> {
        let output_address = page_desc.read(STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB) as usize;
        let page_offset = self.vaddr.get_page_offset_4KiB();
        let phy_addr = (output_address << LEVEL_3_OUTPUT_ADDR_SHIFT) | page_offset;
        let is_cacheable = !page_desc.matches_all(STAGE1_PAGE_DESCRIPTOR::SH::OuterShareable);

        self.res = Some(TranslationDesc {
            virt_addr: self.vaddr,
            phy_addr: PhysicalAddress::new(phy_addr),
            access_perms: parse_access_perms_pd(page_desc),
            memory_kind: if is_cacheable {
                MemoryKind::Normal
            } else {
                MemoryKind::Device
            },
        });

        Ok(TraversalControl::Break)
    }

    fn visit_block_desc_lvl1(
        &mut self,
        block_desc: &Stage1BlockDescriptor,
    ) -> Result<TraversalControl> {
        let output_address = block_desc.read(STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_1GiB) as usize;
        let page_offset = self.vaddr.get_page_offset_1GiB();
        let phy_addr = (output_address << LEVEL_1_OUTPUT_ADDR_SHIFT) | page_offset;

        Ok(self.block_desc_complete_visit(block_desc, phy_addr))
    }

    fn visit_block_desc_lvl2(
        &mut self,
        block_desc: &Stage1BlockDescriptor,
    ) -> Result<TraversalControl> {
        let output_address = block_desc.read(STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_2MiB) as usize;
        let page_offset = self.vaddr.get_page_offset_2MiB();
        let phy_addr = (output_address << LEVEL_2_OUTPUT_ADDR_SHIFT) | page_offset;

        Ok(self.block_desc_complete_visit(block_desc, phy_addr))
    }

    fn visit_invalid(
        &mut self,
        _desc_val: u64,
        _level: &AddressTranslationLevel,
    ) -> Result<TraversalControl> {
        Ok(TraversalControl::Break)
    }
}

struct InsertVisitor<'a, PhyAllocator: Allocator> {
    map: &'a ParsedMemoryMap,
    phy_page_alloc: &'a mut PhyAllocator,
    mmap: &'a MemoryMap,
}

impl<'a, PhyAllocator: Allocator> InsertVisitor<'a, PhyAllocator> {
    fn new(
        map: &'a ParsedMemoryMap,
        phy_page_alloc: &'a mut PhyAllocator,
        mmap: &'a MemoryMap,
    ) -> Self {
        Self {
            map,
            phy_page_alloc,
            mmap,
        }
    }
}

impl<'a, PhyAllocator: Allocator> Visitor for InsertVisitor<'a, PhyAllocator> {
    fn visit_page_desc(&mut self, _: &Stage1PageDescriptor) -> Result<TraversalControl> {
        Err(Error::VMMapExists(*self.mmap))
    }

    fn visit_block_desc_lvl1(&mut self, _: &Stage1BlockDescriptor) -> Result<TraversalControl> {
        Err(Error::VMMapExists(*self.mmap))
    }

    fn visit_block_desc_lvl2(&mut self, _: &Stage1BlockDescriptor) -> Result<TraversalControl> {
        Err(Error::VMMapExists(*self.mmap))
    }

    fn visit_invalid(
        &mut self,
        _desc_val: u64,
        level: &AddressTranslationLevel,
    ) -> Result<TraversalControl> {
        let paddr = self.map.phy_addr.as_raw_ptr() as u64;
        let mut alloc_desc_table = || -> Result<u64> {
            let layout = AllocationLayout::new(1, TRANSLATION_TABLE_DESC_ALIGN, true);
            Ok(self
                .phy_page_alloc
                .allocate_phy_pages(&layout)?
                .start_address()
                .as_raw_ptr() as u64)
        };

        match level {
            AddressTranslationLevel::Zero => {
                let next_level_table = alloc_desc_table()?;
                let tbl_desc = Stage1TableDescriptor::new(new_stage1_table_desc(next_level_table));
                Ok(TraversalControl::UpdateAndContinue(tbl_desc))
            }
            AddressTranslationLevel::One | AddressTranslationLevel::Two => {
                let desc =
                    new_stage1_block_desc(BlockDescLevel::from(level), paddr, self.map.attributes);
                Ok(TraversalControl::UpdateAndBreak(desc))
            }
            AddressTranslationLevel::Three => {
                let desc = new_stage1_page_desc(paddr, self.map.attributes);
                Ok(TraversalControl::UpdateAndBreak(desc))
            }
        }
    }
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
    if ap == STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL1_WRITE
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1_EL0.value {
        AccessPermissions::EL1_READ
            | AccessPermissions::EL1_WRITE
            | AccessPermissions::EL0_READ
            | AccessPermissions::EL0_WRITE
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1.value {
        AccessPermissions::EL1_READ
    } else if ap == STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1_EL0.value {
        AccessPermissions::EL1_READ | AccessPermissions::EL0_READ
    } else {
        bug!("Invalid Access Permissions on page");
    }
}

fn new_stage1_table_desc(next_level_addr: u64) -> u64 {
    let table_desc = Stage1TableDescriptor::new(0);

    table_desc.modify(STAGE1_TABLE_DESCRIPTOR::VALID::True + STAGE1_TABLE_DESCRIPTOR::TYPE::Table);
    table_desc.modify(STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR.val(next_level_addr));

    table_desc.get()
}

fn new_stage1_page_desc(output_address: u64, attributes: u64) -> u64 {
    let page_desc = Stage1PageDescriptor::new(attributes);

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
        BlockDescLevel::One => block_desc.modify(
            STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_1GiB
                .val(output_address >> LEVEL_1_OUTPUT_ADDR_SHIFT),
        ),
        BlockDescLevel::Two => block_desc.modify(
            STAGE1_BLOCK_DESCRIPTOR::OUTPUT_ADDR_2MiB
                .val(output_address >> LEVEL_2_OUTPUT_ADDR_SHIFT),
        ),
    }

    block_desc.get()
}