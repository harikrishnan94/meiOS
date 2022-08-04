use crate::arch_regs::ArchRegisters;
use crate::board_id::{get_raspi_board, RaspiBoard};
use core::mem::size_of;
use core::sync::atomic::{AtomicPtr, Ordering};

fn get_mimo_base() -> *mut u32 {
    const RASPI2_MIMO_BASE: *mut u32 = 0x3F000000 as *mut u32;
    const RASPI4_MIMO_BASE: *mut u32 = 0xFE000000 as *mut u32;
    const RASPIOTHER_MIMO_BASE: *mut u32 = 0x20000000 as *mut u32;

    match get_raspi_board() {
        RaspiBoard::TWO | RaspiBoard::THREE => RASPI2_MIMO_BASE,
        RaspiBoard::FOUR => RASPI4_MIMO_BASE,
        _ => RASPIOTHER_MIMO_BASE,
    }
}

pub struct Mimo {
    base: AtomicPtr<u32>,
}

impl Mimo {
    fn init(&self) {
        if self.base.load(Ordering::Relaxed).is_null() {
            MIMO.base.store(get_mimo_base(), Ordering::Release);
        }
    }

    pub fn base(&self) -> Option<*mut u32> {
        let base = self.base.load(Ordering::Relaxed);
        if !base.is_null() {
            Some(base)
        } else {
            self.init();
            self.base()
        }
    }

    /// # Safety
    pub unsafe fn write<const R: ArchRegisters>(&self, data: u32) {
        core::ptr::write_volatile(
            self.base().unwrap().add(R as usize / size_of::<u32>()),
            data,
        )
    }

    /// # Safety
    pub unsafe fn read<const R: ArchRegisters>(&self) -> u32 {
        core::ptr::read_volatile(self.base().unwrap().add(R as usize / size_of::<u32>()))
    }
}

pub static MIMO: Mimo = Mimo {
    base: AtomicPtr::new(core::ptr::null_mut()),
};
