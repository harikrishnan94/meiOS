// Virtual Memory Layout for Kernel and User space processes
// For kernel, once MMU (paging) is enabled, the entire physical memory range
// is mapped to both TTBR0 and TTBR1.
// So, Initially, Kernel can access the Physical memory from both low and high VA.
// This needed, because once MMU is enabled all accesses go thru MMU and all addresses
// (SP, PC, and other data structures) still point to physical address, which is exactly
// same as low VA (0...PhysicalEnd). So, it is necessary to map both TTBR0 and TTBR1 when
// MMU just enabled for correctness (otherwise SEGFAULT will happen).
// Kernel Virtual Address space is exactly equivalent to Physical Address space.
// And entire Physical address space is mapped to Kernel VA and is available for use always.
// So, In kernel space, working and manipulating with VA is same as doing so in PA.
