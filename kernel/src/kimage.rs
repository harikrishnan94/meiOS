use core::cell::UnsafeCell;

#[allow(improper_ctypes)]
extern "C" {
    /// Provided by Linker
    static __kernel_start_marker: UnsafeCell<()>;
    static __kernel_end_marker: UnsafeCell<()>;
    static __kernel_stack_start_marker: UnsafeCell<()>;
}

pub fn kernel_image_size() -> usize {
    let kstart = unsafe { __kernel_start_marker.get() as usize };
    let kend = unsafe { __kernel_end_marker.get() as usize };

    kend - kstart
}

pub fn kernel_stack_base() -> usize {
    unsafe { __kernel_stack_start_marker.get() as usize }
}
