//                           Generic Slab Allocator
// ---------------------------------------------------------------------------
//
// Slab Allocator provides dynamic allocation and deallocation support for types
// with size upto `MAX_OBJECT_SIZE`.
// Best case allocation and de-allocation runtime is O(1) and worst case is O(log n).
//
//
// Design:
// ------
// Heap (entirety of RAM excluding kernel text and data sections) is managed by SLAB.
// Pages (multiples of 4 KB) are carved out of `Buddy Allocator`.
// For, every 4 KB region, there is a corresponding `Page` metadata. Page metadata
// holds the SLAB metadata for that page.
//
// Bins:
// -----
// SLAB is made up of multiple bins (Defined at compile time).
// Each bin is used to handle allocations/deallocations for a particular `Layout`.
// Each bin has list of pages of specific size, determined by object size.
// Pages consists of arrays of objects, that are returned to caller.
// Once an object is allocated, it cannot be given to another caller, before deallocated.
// Pages are not returned back to Buddy Allocator, unless all objects inside the page are free.
//
// Fragmentation:
// -------------
// Bins are designed (object size -> page size) in such a way that there is at worst 12.5 %
// internal fragmentation.
// Since, objects of sizes in range (prev bin object size..current bin object size) are all allocated
// from the current bin, limiting the worst case fragmentation is necessary.
// For object sizes that are not multiple of page size, the worst case wastage (external fragmentation)
// limited to 3.125 %.
//
// Best case O(1) Allocation and Deallocation:
// ------------------------------------------
// During allocation, identifying the `Bin` for the requested size is performed by a looking up a table,
// generated in build time.
// During deallocation, identifying the `Bin` and `Page` metadata for the requested pointer is done by,
// 1) Down round the pointer to the min page size (4 KB) and read the corresponding page's metadata.
// 2) Page metadata contains the `Bin` information (contains page size for the Bin).
// 3) Down round the pointer to the page size of the `Bin`.
// 4) Then add the pointer the free-list for the page.
//
// Finding free pages:
// ------------------
// When current page for the bin becomes full (all objects are allocated), new page for the bin needs to be
// allocated/selected.
// Two strategies are available for selecting a new page.
// a) Choose a partially filled page with most amount of free objects.
//     - Best for overall performance.
//     - Subsequent allocations returns nearby blocks - Improves cache usage
//     - SlabPool cache hit also increased due to using the same metadata over multiple allocations.
// b) Choose a partially filled page with least amount of free objects.
//     - Overall Memory Efficient.
//     - Best for avoiding fragmentation (External)
//     - Facilitates earlier release of free'd block to upstream allocator.
// In either case if no such partial page can be found, a fresh page is allocated using upstream allocator.
//
// Sorting Partially Filled Pages:
// ------------------------------
// Instead of fully sorting the pages using the amount of free objects available,
// approximate sorting can be done using `approximate-range-sort`.
// 5 Levels (Watermarks) - used to approximate sorted blocks.
//     - 5 levels after - full (no free blocks) pages.
//     - Atleast 1 free block to 20 % blocks free.
//     - 20 - 40 % blocks free.
//     - 40 - 60 % blocks free.
//     - 60 - 80 % blocks free.
//     - 80 - 100 % blocks free.
// After each free operation, a page is moved from one level to another, if it crosses the watermark.
// Once, the page becomes completely full (after allocation), it is moved to full queue.
use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    fmt::Debug,
    ops::Range,
    ptr::{null, null_mut, NonNull},
};

use crate::{address::PhysicalAddress, error::Result, mmu};
use heapless::Vec;
use macros::ctor;
use modular_bitfield::prelude::*;
use spin::mutex::Mutex;

#[derive(Debug)]
#[repr(transparent)]
struct Page {
    hdr: PageHdr,
}

#[derive(Clone, Copy)]
#[bitfield(bits = 128)]
struct PageHdr {
    prev: B48,
    next: B48,
    num_free: B12,
    free_list: B12,
    slab_bin_id: B8,
}

impl PageHdr {
    fn is_linked(&self) -> bool {
        debug_assert!(if self.next() != 0 {
            self.prev() != 0
        } else {
            self.prev() == 0
        });
        self.next() != 0
    }

    fn unlink(&mut self) {
        debug_assert!(self.is_linked());

        let next = self.next();
        let prev = self.prev();

        unsafe {
            (*(next as *mut PageHdr)).set_prev(prev);
            (*(prev as *mut PageHdr)).set_next(next);
        }
    }

    fn get_next_link(&self) -> *const PageHdr {
        self.next() as *const PageHdr
    }
    fn get_next_link_mut(&mut self) -> *mut PageHdr {
        self.next() as *mut PageHdr
    }

    fn get_prev_link(&self) -> *const PageHdr {
        self.prev() as *const PageHdr
    }
    fn get_prev_link_mut(&mut self) -> *mut PageHdr {
        self.prev() as *mut PageHdr
    }

    fn set_next_link(&mut self, next: *const PageHdr) {
        debug_assert!(next as u64 == next as u64 & !(u64::MAX >> 16));
        self.set_next(next as u64);
    }

    fn set_prev_link(&mut self, prev: *const PageHdr) {
        debug_assert!(prev as u64 == prev as u64 & !(u64::MAX >> 16));
        self.set_prev(prev as u64);
    }
}

struct PageHdrList {
    head: *mut Page,
    tail: *mut Page,
}

unsafe impl Send for PageHdrList {}

impl PageHdrList {
    const fn new() -> Self {
        PageHdrList {
            head: null_mut(),
            tail: null_mut(),
        }
    }

    fn push_back(&mut self, page: *mut Page) {
        unsafe {
            let hdr = &mut (*page).hdr;

            hdr.set_next_link(null());
            hdr.set_prev_link(&(*self.tail).hdr);
        }
        self.tail = page;
        if self.head.is_null() {
            self.head = page;
        }
    }

    fn pop_back(&mut self) -> Option<*mut Page> {
        if self.is_empty() {
            return None;
        }

        let page = self.tail;
        self.tail = unsafe {
            let prev = (*page).hdr.get_prev_link() as *mut Page;
            (*prev).hdr.set_next_link(null());
            (*page).hdr.set_prev_link(null());
            debug_assert!((*page).hdr.get_next_link().is_null());
            prev
        };

        if self.tail.is_null() {
            self.head = null_mut();
        }

        Some(page)
    }

    fn pop_front(&mut self) -> Option<*mut Page> {
        if self.is_empty() {
            return None;
        }

        let page = self.head;
        self.head = unsafe {
            let next = (*page).hdr.get_next_link() as *mut Page;
            (*next).hdr.set_prev_link(null());
            (*page).hdr.set_next_link(null());
            debug_assert!((*page).hdr.get_prev_link().is_null());
            next
        };

        if self.head.is_null() {
            self.tail = null_mut();
        }

        Some(page)
    }

    fn is_empty(&self) -> bool {
        if self.head.is_null() {
            debug_assert!(self.tail.is_null());
            return true;
        }

        debug_assert!(!self.tail.is_null());
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct BinDesc {
    obj_size: u16,
    page_size: u16,
}

const NUM_PARTIAL_PAGES_LEVELS: usize = 5;

pub struct Pool {
    bin_desc: BinDesc,
    cur_page: Option<&'static mut Page>,
    full_pages: PageHdrList,
    partial_pages: Vec<PageHdrList, NUM_PARTIAL_PAGES_LEVELS>,
}

impl Pool {
    fn new(bin_desc: BinDesc) -> Self {
        let mut partial_pages = Vec::new();

        for _ in 0..NUM_PARTIAL_PAGES_LEVELS {
            partial_pages
                .push(PageHdrList::new())
                .map_err(|_| ())
                .unwrap();
        }

        Self {
            bin_desc,
            cur_page: None,
            full_pages: PageHdrList::new(),
            partial_pages,
        }
    }

    fn alloc(&mut self) -> Option<&'static [u8]> {
        todo!()
    }

    fn free(&mut self, ptr: *mut u8) -> bool {
        todo!()
    }
}

const MAX_BINS: usize = 100;

struct Slab {
    bins: Vec<Mutex<Pool>, MAX_BINS>,
}

const MIN_ALIGN_SIZE: usize = 16;
const MAX_OBJECT_SIZE: usize = 2048;

const fn bin_count(max_alloc_size: usize) -> usize {}

impl Slab {
    fn new() -> Self {
        Self { bins: Vec::new() }
    }
}

/// Token Type for Global Allocator
pub struct GlobalSlab;

impl GlobalSlab {
    const fn new() -> Self {
        Self {}
    }
}

#[ctor]
static ALLOCATOR_IMPL: Slab = Slab::new();

/// Global Allocator used by `alloc` crate.
/// Relies on Slab for allocation/deallocation.
#[global_allocator]
static ALLOCATOR: GlobalSlab = GlobalSlab::new();

unsafe impl Allocator for Slab {
    fn allocate(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        todo!()
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        todo!()
    }
}

unsafe impl GlobalAlloc for GlobalSlab {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok(ptr) = ALLOCATOR_IMPL.allocate(layout) {
            ptr.as_mut_ptr()
        } else {
            null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATOR_IMPL.deallocate(NonNull::new(ptr).unwrap(), layout)
    }
}

pub fn init(mem: Range<PhysicalAddress>, min_page_size: usize) -> Result<()> {
    let usable_mem = init_metadata(mem, min_page_size)?;
    init_buddy_allocator(usable_mem, min_page_size)
}

/// Initialize Slab Metadata and return range of physical address for physical page
///
/// # Errors
///
/// This function will return an error if SLAB of `min_page_size` cannot be initialized within the
/// given physical memory region
fn init_metadata(
    mem: Range<PhysicalAddress>,
    min_page_size: usize,
) -> Result<Range<PhysicalAddress>> {
    todo!()
}

fn init_buddy_allocator(mem: Range<PhysicalAddress>, min_page_size: usize) -> Result<()> {
    todo!()
}

// pub fn get_page_allocator() -> &'static impl Allocator {
//     todo!()
// }

impl Debug for PageHdr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let prev = self.get_prev_link();
        let next = self.get_next_link();
        let num_free = self.num_free();
        let free_list = self.free_list();
        let slab_bin_id = self.slab_bin_id();

        f.debug_struct("PageHdr")
            .field("prev", &prev)
            .field("next", &next)
            .field("num_free", &num_free)
            .field("free_list", &free_list)
            .field("slab_bin_id", &slab_bin_id)
            .finish()
    }
}
