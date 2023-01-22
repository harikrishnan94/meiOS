use crate::vm::MemoryMap;

#[derive(Debug, Clone, Copy)]
pub enum Error {
    BumpAllocatorOOM(usize),
    InvalidVirtualAddress(usize),

    CorruptedTranslationTable(u64),
    VMMapExists(MemoryMap),
    VMMapNotExists(MemoryMap),

    PhysicalOOM,
    ContigiousPhysicalRangeUnavailable(u64),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BumpAllocatorOOM(max_size) => {
                write!(f, "Static Bump Allocator OOM. Configured Size = {max_size}")
            }

            Error::InvalidVirtualAddress(addr) => write!(f, "Invalid Virtual Address `{addr}`"),

            Error::CorruptedTranslationTable(tt_desc) => {
                write!(f, "Translation Table Corrupted @ 0x{tt_desc:X}")
            }
            Error::VMMapExists(map) => write!(f, "Provided Map already Exists: {map}"),
            Error::VMMapNotExists(map) => write!(f, "Requested Map doesn't Exist: {map}"),

            Error::PhysicalOOM => write!(f, "Out of Physical Memory"),
            Error::ContigiousPhysicalRangeUnavailable(num_pages) => {
                write!(
                    f,
                    "Contigious Physical Memory Range Unavailable for {num_pages} pages"
                )
            }
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
