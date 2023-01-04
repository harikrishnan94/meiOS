use crate::address::PhysicalAddress;

#[derive(Debug, Clone, Copy)]
pub enum Error {
    BumpAllocatorOOM(usize),
    InvalidVirtualAddress(usize),
    PhysicalAddressNotStaticallyMapped(PhysicalAddress),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BumpAllocatorOOM(max_size) => write!(
                f,
                "Static Bump Allocator OOM. Configured Size = {}",
                max_size
            ),
            Error::InvalidVirtualAddress(addr) => write!(f, "Invalid Virtual Address `{}`", addr),
            Error::PhysicalAddressNotStaticallyMapped(paddr) => write!(
                f,
                "Physical Address(`{}`) is not statically mapped.
            Peripheral, Kernel image addresses are statically mapped, for example",
                paddr
            ),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
