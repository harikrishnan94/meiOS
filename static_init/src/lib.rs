#![no_std]
#![feature(core_intrinsics)]

use core::{
    cell::UnsafeCell,
    intrinsics::unlikely,
    marker::Sync,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

pub struct StaticInitialized<T: Sync> {
    is_initialized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
    initializer: fn() -> T,
    init_priority: u8,
}

unsafe impl<T: Sync> Sync for StaticInitialized<T> {}

impl<T: Sync> StaticInitialized<T> {
    pub const fn new(initializer: fn() -> T, init_priority: u8) -> Self {
        Self {
            is_initialized: AtomicBool::new(false),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            initializer,
            init_priority,
        }
    }

    #[inline(always)]
    pub fn get(&self) -> &T {
        if unlikely(!self.is_initialized.load(Ordering::Relaxed)) {
            self.init_immediate();
        }
        unsafe { (*self.data.get()).assume_init_ref() }
    }

    #[inline(never)]
    pub fn init_immediate(&self) {
        if CURRENT_INIT_PRIORITY.load(Ordering::Relaxed) < self.init_priority {
            panic!("higher priority object initialization is dependent on lower priority object's initialization");
        }
        unsafe {
            (*self.data.get()).as_mut_ptr().write((self.initializer)());
        }
        self.is_initialized.store(true, Ordering::Relaxed);
    }

    #[inline(never)]
    pub fn init(&self) {
        if !self.is_initialized.load(Ordering::Relaxed)
            && self.init_priority == CURRENT_INIT_PRIORITY.load(Ordering::Relaxed)
        {
            self.init_immediate();
        }
    }
}

impl<T: Sync> core::ops::Deref for StaticInitialized<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        self.get()
    }
}

pub static CURRENT_INIT_PRIORITY: AtomicU8 = AtomicU8::new(0);
pub const MAX_INIT_PRIORITY: u8 = 4;
