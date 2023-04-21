#include "libmei/src/vm/slab.rs.h"

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
// Since, objects of sizes in range (prev bin object size..current bin object size) are all
// allocated from the current bin, limiting the worst case fragmentation is necessary. For object
// sizes that are not multiple of page size, the worst case wastage (external fragmentation) limited
// to 3.125 %.
//
// Best case O(1) Allocation and Deallocation:
// ------------------------------------------
// During allocation, identifying the `Bin` for the requested size is performed by a looking up a
// table, generated in build time. During deallocation, identifying the `Bin` and `Page` metadata
// for the requested pointer is done by, 1) Down round the pointer to the min page size (4 KB) and
// read the corresponding page's metadata. 2) Page metadata contains the `Bin` information (contains
// page size for the Bin). 3) Down round the pointer to the page size of the `Bin`. 4) Then add the
// pointer the free-list for the page.
//
// Finding free pages:
// ------------------
// When current page for the bin becomes full (all objects are allocated), new page for the bin
// needs to be allocated/selected. Two strategies are available for selecting a new page. a) Choose
// a partially filled page with most amount of free objects.
//     - Best for overall performance.
//     - Subsequent allocations returns nearby blocks - Improves cache usage
//     - SlabPool cache hit also increased due to using the same metadata over multiple allocations.
// b) Choose a partially filled page with least amount of free objects.
//     - Overall Memory Efficient.
//     - Best for avoiding fragmentation (External)
//     - Facilitates earlier release of free'd block to upstream allocator.
// In either case if no such partial page can be found, a fresh page is allocated using upstream
// allocator.
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
// After each free operation, a page is moved from one level to another, if it crosses the
// watermark. Once, the page becomes completely full (after allocation), it is moved to full queue.

namespace mei::vm::slab {
struct PageHdr {};

auto Init(uint8_t* phy_start,
          uint8_t* phy_end,
          rust::Fn<UpstreamAllocator(uint8_t* size, uint8_t* align)> initalize_upstream,
          rust::Fn<void(UpstreamAllocator, size_t size)> alloc_page,
          rust::Fn<void(UpstreamAllocator, uint8_t* ptr, size_t size)> free_page) -> bool {}

auto Alloc(const Layout& layout) -> AllocResult {}

auto Free(uint8_t* ptr, const Layout& layout) -> AllocResult {}

auto GetUpstreamAllocator() -> UpstreamAllocator {}
}  // namespace mei::vm::slab