use core::cell::UnsafeCell;

use libmei::address::PhysicalAddress;

#[allow(improper_ctypes)]
extern "C" {
    /// Provided by Linker
    static __kernel_start_marker: UnsafeCell<()>;
    static __kernel_end_marker: UnsafeCell<()>;
    static __kernel_stack_start_marker: UnsafeCell<()>;
}

pub fn kernel_phy_range() -> core::ops::Range<PhysicalAddress> {
    let kstart = unsafe { __kernel_start_marker.get() as usize };
    let kend = unsafe { __kernel_end_marker.get() as usize };
    PhysicalAddress::new(kstart)..PhysicalAddress::new(kend)
}

#[allow(dead_code)]
pub fn kernel_stack_range() -> core::ops::Range<PhysicalAddress> {
    let stack_top = unsafe { __kernel_stack_start_marker.get() as usize };
    let stack_bottom = 0;
    PhysicalAddress::new(stack_bottom)..PhysicalAddress::new(stack_top)
}

pub fn kernel_image_size() -> usize {
    let krange = kernel_phy_range();
    (krange.end - krange.start) as usize
}

pub fn kernel_stack_base() -> usize {
    unsafe { __kernel_stack_start_marker.get() as usize }
}
