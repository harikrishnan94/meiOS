#[derive(Debug, Clone, Copy)]
pub enum Error {
    BumpAllocatorOOM(usize),
    InvalidVirtualAddress(usize),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BumpAllocatorOOM(max_size) => {
                write!(f, "Static Bump Allocator OOM. Configured Size = {max_size}")
            }
            Error::InvalidVirtualAddress(addr) => write!(f, "Invalid Virtual Address `{addr}`"),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
