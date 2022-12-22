use core::{
    mem::{size_of, MaybeUninit},
    sync::atomic::{AtomicUsize, Ordering},
};

const MEMORY_REGION_LEN: usize = 8192;

/// Initialized by ASM
#[no_mangle]
static MEMORY_REGION_BASE: usize = 0;

pub struct BumpAllocator {
    memory_region_len: usize,
    memory_region_base: usize,
    memory_region_used: AtomicUsize,
}

impl BumpAllocator {
    fn default() -> Self {
        BumpAllocator {
            memory_region_base: MEMORY_REGION_BASE,
            memory_region_len: MEMORY_REGION_LEN,
            memory_region_used: AtomicUsize::new(0),
        }
    }

    pub fn allocate<'a, T: Sized>(&self, val: T) -> Option<&'a mut T> {
        loop {
            let used = self.memory_region_used.load(Ordering::Acquire);
            let new = used + size_of::<T>();

            if new < self.memory_region_len {
                if self
                    .memory_region_used
                    .compare_exchange_weak(used, new, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    unsafe {
                        let ptr = ((self.memory_region_base + used) as *mut MaybeUninit<T>)
                            .as_mut()
                            .unwrap();
                        ptr.write(val);
                        return Some(ptr.assume_init_mut());
                    }
                }
            } else {
                return None;
            }
        }
    }
}

lazy_static! {
    pub static ref BUMP_ALLOCATOR: BumpAllocator = BumpAllocator::default();
}
