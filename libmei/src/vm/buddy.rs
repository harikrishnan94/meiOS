use core::{
    alloc::{AllocError, Allocator, Layout},
    cmp::{max, min},
    mem::size_of,
    ops::Range,
    ptr::NonNull,
    slice,
};

use spin::{Mutex, MutexGuard};

#[cfg(test)]
use std::vec::Vec;

use crate::{
    address::{Address, PhysicalAddress},
    error::{Error, Result},
};

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

/// `BuddyAllocator` manages a region of physical memory and allows
/// caller to obtain `power of 2` sized blocks out of it.
pub struct BuddyAllocator {
    storage: &'static Storage,
}

impl BuddyAllocator {
    /// Creates a `BuddyAllocator` which manages the provided physical address
    /// range.
    ///
    /// # Safety
    ///
    /// `mem` range should be pointing to a valid Physical Address range,
    /// that is *NOT* currently in use.
    /// After this call, the provided physical memory range is OWNED by
    /// the allocator and any direct use of the memory range, that is not
    /// obtained by `alloc` is Undefined
    pub unsafe fn manage(
        mem: Range<PhysicalAddress>,
        min_alloc_size: usize,
        max_alloc_size: usize,
    ) -> Option<Self> {
        let storage = Storage::init(&mem, min_alloc_size, max_alloc_size)?;
        Some(Self { storage })
    }

    /// Return an Unique physical address range of the desired size.
    ///
    /// # Safety
    ///
    /// `self` must have been created using `manage`.
    pub unsafe fn alloc(&self, size: usize) -> Result<PhysicalAddress> {
        if !size.is_power_of_two() {
            return Err(Error::AllocError);
        }

        let start_level = max(size.ilog2(), self.storage.min_level);
        for level in start_level..=self.storage.max_level {
            let mut free_area = self.storage.get_free_area(level);

            if !free_area.free_list.is_empty() {
                let block = free_area.free_list.pop_back().unwrap();
                free_area.mark_used(level, block, self.storage.zero_page);
                drop(free_area);

                let mut level = level;
                while level != start_level {
                    level -= 1;

                    let buddy = self.storage.get_buddy(block, level);
                    FreeBlock::init(buddy as *const FreeBlock as usize as *mut FreeBlock);

                    let mut free_area = self.storage.get_free_area(level);
                    free_area.free_list.push_back(buddy);

                    free_area.mark_used(level, block, self.storage.zero_page);
                }

                return Ok(PhysicalAddress::new(block as *const FreeBlock as usize));
            }
        }

        Err(Error::PhysicalOOM)
    }

    /// Frees the `ptr`. `ptr` will be reused for future allocations.
    ///
    /// # Safety
    ///
    /// `self` must have been created using `manage`.
    /// `ptr` must have been allocated using this buddy allocator for the `size`.
    pub unsafe fn free(&self, ptr: PhysicalAddress, size: usize) -> Result<()> {
        let start_level = max(size.ilog2(), self.storage.min_level);

        if self.storage.start_page > ptr
            || self.storage.end_page < ptr + size
            || !size.is_power_of_two()
            || start_level > self.storage.max_level
        {
            return Err(Error::AllocError);
        }

        let mut block = &*(ptr.as_raw_ptr() as *const FreeBlock);
        for level in start_level..=self.storage.max_level {
            let mut free_area = self.storage.get_free_area(level);

            let buddy_free = free_area.mark_free(level, block, self.storage.zero_page);
            if !buddy_free {
                FreeBlock::init(block as *const FreeBlock as usize as *mut FreeBlock);
                free_area.free_list.push_back(block);
                break;
            }

            let buddy = self.storage.get_buddy(block, level);
            assert!(buddy.link.is_linked());
            let mut cursor = free_area
                .free_list
                .cursor_mut_from_ptr(buddy as *const FreeBlock);
            cursor.remove().unwrap();

            block = Storage::get_smaller(block, buddy);
        }

        Ok(())
    }

    #[cfg(test)]
    /// Returns a list of # of blocks for each size that are free
    /// (size, count)
    unsafe fn get_free_area_information(&self) -> Vec<(usize, usize)> {
        let mut free_area_info = Vec::new();
        for level in self.storage.min_level..=self.storage.max_level {
            let free_area = self.storage.get_free_area(level);
            let count = free_area.free_list.iter().count();
            free_area_info.push((1 << level, count));
        }
        return free_area_info;
    }
}

#[repr(C)]
#[repr(align(64))]
struct Storage {
    zero_page: PhysicalAddress,
    start_page: PhysicalAddress,
    end_page: PhysicalAddress,
    min_level: u32,
    max_level: u32,
    free_areas: &'static [FreeAreaMutex],
    // FreeArea and FreeMap memory
}

type FreeAreaMutex = Mutex<FreeArea>;

impl Storage {
    unsafe fn add(&self, level: u32, mem: Range<PhysicalAddress>) {
        let mem_start = mem.start;
        let mem_end = mem.end;
        let level_size = 1usize << level;
        let offset = mem_start.align_offset(level_size);
        let mem_start_cur_level = min(
            PhysicalAddress::new(mem_start.align_up(level_size)),
            mem_end,
        );

        if offset != 0 {
            self.add(level - 1, mem_start..mem_start_cur_level);
        }

        let mem_start = mem_start_cur_level;
        if mem_start >= mem_end {
            return;
        }

        assert!(mem_start.is_aligned(level_size));

        let mut count = 0;
        if mem_start + (count + 1) * level_size <= mem_end {
            let mut free_area = self.get_free_area(level);
            while mem_start + (count + 1) * level_size <= mem_end {
                let block = (mem_start + count * level_size).as_raw_ptr() as *mut FreeBlock;

                assert!((block as usize) < mem_end.as_raw_ptr());

                FreeBlock::init(block);
                free_area.free_list.push_back(&*block);
                free_area.mark_free(level, &*block, self.zero_page);

                count += 1;
            }
        }

        let mem_start = mem_start + count * level_size;
        assert!(mem_start <= mem_end);
        if mem_start != mem_end {
            self.add(level - 1, mem_start..mem_end)
        }
    }

    unsafe fn get_free_area(&self, level: u32) -> MutexGuard<FreeArea> {
        let level = level - self.min_level;
        self.free_areas[level as usize].lock()
    }

    unsafe fn get_buddy(&self, block: &FreeBlock, level: u32) -> &FreeBlock {
        &*(self.get_buddy_ptr(block, level) as *const FreeBlock)
    }

    unsafe fn get_buddy_ptr(&self, block: &FreeBlock, level: u32) -> usize {
        let buddy_ptr = ((block as *const FreeBlock as usize >> level) ^ 1) << level;
        assert!(buddy_ptr < self.end_page.as_raw_ptr());
        buddy_ptr
    }

    fn get_smaller(block: &'static FreeBlock, buddy: &'static FreeBlock) -> &'static FreeBlock {
        let block_ptr = block as *const FreeBlock as usize;
        let buddy_ptr = buddy as *const FreeBlock as usize;

        assert_ne!(block_ptr, buddy_ptr);

        if block_ptr < buddy_ptr {
            block
        } else {
            buddy
        }
    }

    unsafe fn init(
        mem: &Range<PhysicalAddress>,
        min_alloc_size: usize,
        max_alloc_size: usize,
    ) -> Option<&'static mut Self> {
        let min_alloc_size = min_alloc_size.next_power_of_two();
        let max_alloc_size = max_alloc_size.next_power_of_two();

        assert!(min_alloc_size >= size_of::<FreeBlock>());
        assert!(max_alloc_size > min_alloc_size);

        let min_level = min_alloc_size.ilog2();
        let max_level = max_alloc_size.ilog2();
        let num_levels = max_alloc_size.ilog2() - min_level + 1;
        let end_page = PhysicalAddress::new(mem.end.align_down(min_alloc_size));
        let mut alloc_start = mem.start;

        let this = Self::claim_memory::<Self>(&mut alloc_start, end_page, 1)?;
        let free_areas =
            Self::claim_memory::<FreeAreaMutex>(&mut alloc_start, end_page, num_levels as usize)?;

        for level in min_level..=max_level {
            let free_map = Self::claim_memory::<u8>(
                &mut alloc_start,
                end_page,
                FreeArea::free_map_size_for(level, max_level),
            )?;
            let free_map = FreeArea::init_free_map(free_map, level, max_level);

            let free_area = free_areas.add((level - min_level) as usize);
            FreeArea::init(free_area, free_map);
        }

        alloc_start = PhysicalAddress::new(alloc_start.align_up(min_alloc_size));

        this.write(Self {
            zero_page: PhysicalAddress::new(alloc_start.align_down(max_alloc_size)),
            start_page: alloc_start,
            end_page,
            min_level,
            max_level,
            free_areas: slice::from_raw_parts_mut(free_areas, num_levels as usize),
        });

        let this = &mut *this;
        this.add(max_level, this.start_page..this.end_page);

        Some(this)
    }

    fn claim_memory<T: Sized>(
        ptr: &mut PhysicalAddress,
        end: PhysicalAddress,
        n: usize,
    ) -> Option<*mut T> {
        let layout = Layout::new::<T>().repeat(n).unwrap().0;
        let mem = ptr.align_up(layout.align());

        if mem + layout.size() >= end.as_raw_ptr() {
            return None;
        }

        *ptr = PhysicalAddress::new(mem + layout.size());

        Some(mem as *mut T)
    }
}

#[repr(C)]
struct FreeBlock {
    link: LinkedListLink,
}

impl FreeBlock {
    unsafe fn init(this: *mut Self) -> &'static FreeBlock {
        this.write(FreeBlock {
            link: LinkedListLink::new(),
        });
        &mut *this
    }
}

intrusive_adapter!(FreeBlockAdapter<'a> = &'a FreeBlock: FreeBlock { link: LinkedListLink });

type FreeList<'a> = LinkedList<FreeBlockAdapter<'a>>;

#[repr(C)]
#[repr(align(64))]
struct FreeArea {
    free_list: FreeList<'static>,
    map: &'static mut [u8],
}

type FreeMap = [u8];

impl FreeArea {
    unsafe fn init(this: *mut FreeAreaMutex, map: &'static mut [u8]) {
        this.write(Mutex::new(Self {
            free_list: FreeList::default(),
            map,
        }));
    }

    unsafe fn init_free_map(map: *mut u8, level: u32, max_level: u32) -> &'static mut FreeMap {
        let map_size = Self::free_map_size_for(level, max_level);
        let map = slice::from_raw_parts_mut(map, map_size);

        for v in map.iter_mut() {
            *v = 0;
        }

        map
    }

    fn free_map_size_for(level: u32, max_level: u32) -> usize {
        let bit_count = (1usize << (max_level - level)).next_multiple_of(8);
        bit_count / 8
    }

    unsafe fn mark_used(&mut self, level: u32, block: &FreeBlock, zero_page: PhysicalAddress) {
        self.flip_bit(level, block, zero_page);
    }

    unsafe fn mark_free(
        &mut self,
        level: u32,
        block: &FreeBlock,
        zero_page: PhysicalAddress,
    ) -> bool {
        self.flip_bit(level, block, zero_page)
    }

    unsafe fn flip_bit(
        &mut self,
        level: u32,
        block: &FreeBlock,
        zero_page: PhysicalAddress,
    ) -> bool {
        let (word, mask) = self.get_word_and_mask(
            level,
            block as *const FreeBlock as usize - zero_page.as_raw_ptr(),
        );
        *word ^= mask;
        *word & mask == 0
    }

    unsafe fn get_word_and_mask(&mut self, level: u32, addr: usize) -> (&mut u8, u8) {
        let idx = (addr >> level) / 2;
        let word = idx / 8;
        let bit = idx % 8;
        (&mut self.map[word], 1 << bit)
    }
}

unsafe impl Sync for BuddyAllocator {}
unsafe impl Send for BuddyAllocator {}

unsafe impl Allocator for BuddyAllocator {
    fn allocate(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        let size = max(layout.size().next_power_of_two(), layout.align());
        unsafe {
            let paddr = self.alloc(size).map_err(|_| AllocError {})?;
            Ok(NonNull::slice_from_raw_parts(
                NonNull::new_unchecked(paddr.as_mut_ptr::<u8>()),
                layout.size(),
            ))
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = max(layout.size().next_power_of_two(), layout.align());
        self.free(PhysicalAddress::new(ptr.as_ptr() as usize), size)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::{
        alloc::{Allocator, Layout},
        ptr::NonNull,
    };
    use rand::{seq::SliceRandom, thread_rng};
    use std::{
        boxed::Box,
        collections::{HashMap, HashSet},
        vec::Vec,
    };

    use crate::address::PhysicalAddress;

    use super::BuddyAllocator;

    fn get_alloc_sizes(alloc: &BuddyAllocator) -> Vec<usize> {
        let mut alloc_sizes = Vec::new();

        for (size, count) in unsafe { alloc.get_free_area_information() } {
            alloc_sizes.append(&mut (0..count).map(|_| size).collect());
        }

        alloc_sizes.shuffle(&mut thread_rng());
        alloc_sizes
    }

    #[test]
    fn buddy_sanity_test() {
        const min_alloc_size: usize = 16;
        const max_alloc_size: usize = 64 * 1024;

        let chunk = Box::new([0xfeu8; max_alloc_size]);
        let mem_start = PhysicalAddress::new(chunk.as_ptr() as usize);
        let mem_end = mem_start + max_alloc_size;
        let allocator =
            unsafe { BuddyAllocator::manage(mem_start..mem_end, min_alloc_size, max_alloc_size) };

        assert!(allocator.is_some());

        let allocator = allocator.unwrap();
        let alloc_sizes = get_alloc_sizes(&allocator);

        // Verify if all the blocks can be allocated.
        let mut ptrs = HashMap::new();
        for alloc_size in alloc_sizes {
            let layout = Layout::from_size_align(alloc_size, alloc_size).unwrap();
            let mem = unsafe { allocator.allocate(layout) };
            assert!(mem.is_ok());

            let mem = mem.unwrap();
            let ptr = unsafe { (*mem.as_ptr()).as_ptr() as usize };

            assert_ne!(ptr, 0);
            assert_eq!(ptr % layout.align(), 0);
            assert!(!ptrs.contains_key(&ptr));

            randomize_mem(mem);
            ptrs.insert(ptr, layout);
        }

        // Verify that any more call to alloc returns error.
        let mem = unsafe {
            allocator.allocate(Layout::from_size_align_unchecked(
                min_alloc_size,
                min_alloc_size,
            ))
        };
        assert!(mem.is_err());

        // Free all ptrs
        for (ptr, layout) in ptrs {
            let mem = unsafe { NonNull::from(&*(ptr as *const u8)) };
            unsafe { allocator.deallocate(mem, layout) };
        }
    }

    #[test]
    fn buddy_coalesce_test() {
        const min_alloc_size: usize = 16;
        const max_alloc_size: usize = 64 * 1024;

        let chunk = Box::new([0xfeu8; max_alloc_size]);
        let mem_start = PhysicalAddress::new(chunk.as_ptr() as usize);
        let mem_end = mem_start + max_alloc_size;
        let allocator =
            unsafe { BuddyAllocator::manage(mem_start..mem_end, min_alloc_size, max_alloc_size) };

        assert!(allocator.is_some());

        let allocator = allocator.unwrap();
        let free_area_info = unsafe { allocator.get_free_area_information() };
        let alloc_count = free_area_info.iter().fold(0usize, |sum, (size, count)| {
            sum + (size / min_alloc_size) * count
        });
        let layout = Layout::from_size_align(min_alloc_size, min_alloc_size).unwrap();

        let mut ptrs = HashSet::new();
        for _ in 0..alloc_count {
            let mem = unsafe { allocator.allocate(layout) };
            assert!(mem.is_ok());

            let mem = mem.unwrap();
            let ptr = unsafe { (*mem.as_ptr()).as_ptr() as usize };

            assert_ne!(ptr, 0);
            assert_eq!(ptr % layout.align(), 0);
            assert!(!ptrs.contains(&ptr));

            randomize_mem(mem);

            ptrs.insert(ptr);
        }

        // Free all ptrs
        for ptr in ptrs {
            let mem = unsafe { NonNull::from(&*(ptr as *const u8)) };
            unsafe { allocator.deallocate(mem, layout) };
        }

        let free_area_info_later = unsafe { allocator.get_free_area_information() };
        assert_eq!(free_area_info, free_area_info_later);
    }

    fn randomize_mem(mut mem: NonNull<[u8]>) {
        let mem = unsafe { mem.as_mut() };
        for v in mem.iter_mut() {
            *v = 0xfe;
        }
    }
}
