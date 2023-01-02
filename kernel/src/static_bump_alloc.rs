use core::{
    mem::{size_of, MaybeUninit},
    sync::atomic::{AtomicUsize, Ordering},
};

const DEF_MEMORY_REGION_LEN: usize = 8192;

pub struct BumpAllocator<const MEMORY_REGION_LEN: usize> {
    memory_region: [u8; MEMORY_REGION_LEN],
    memory_region_used: AtomicUsize,
}

impl<const MEMORY_REGION_LEN: usize> BumpAllocator<MEMORY_REGION_LEN> {
    fn new() -> Self {
        BumpAllocator {
            memory_region: [0; MEMORY_REGION_LEN],
            memory_region_used: AtomicUsize::new(0),
        }
    }

    pub fn allocate<'a, T: Sized>(&self, val: T) -> Option<&'a mut T> {
        loop {
            let used = self.memory_region_used.load(Ordering::Acquire);
            let new = used + size_of::<T>();

            if new < MEMORY_REGION_LEN {
                if self
                    .memory_region_used
                    .compare_exchange_weak(used, new, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    unsafe {
                        let ptr = ((self.memory_region_base() + used) as *mut MaybeUninit<T>)
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

    fn memory_region_base(&self) -> usize {
        self.memory_region.as_ptr() as usize
    }
}

impl<const MEMORY_REGION_LEN: usize> Default for BumpAllocator<MEMORY_REGION_LEN> {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static! {
    pub static ref BUMP_ALLOCATOR: BumpAllocator<DEF_MEMORY_REGION_LEN> = BumpAllocator::default();
}
