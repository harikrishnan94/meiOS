#![no_std]
#![feature(core_intrinsics)]

use core::{
    cell::UnsafeCell,
    intrinsics::unlikely,
    marker::Sync,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct StaticInitialized<T: Sync> {
    is_initialized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
    initializer: fn() -> T,
}

unsafe impl<T: Sync> Sync for StaticInitialized<T> {}

impl<T: Sync> StaticInitialized<T> {
    pub const fn new(initializer: fn() -> T) -> Self {
        Self {
            is_initialized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            initializer,
        }
    }

    #[inline(always)]
    pub fn get(&self) -> &T {
        if unlikely(!self.is_initialized.load(Ordering::Relaxed)) {
            self.init();
        }
        unsafe { (*self.data.get()).assume_init_ref() }
    }

    #[inline(never)]
    pub fn init(&self) {
        unsafe {
            (*self.data.get()).as_mut_ptr().write((self.initializer)());
        }
        self.is_initialized.store(true, Ordering::Relaxed);
    }
}

impl<T: Sync> core::ops::Deref for StaticInitialized<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        self.get()
    }
}
