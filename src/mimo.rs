use crate::arch_regs::ArchRegisters;
use crate::board_id::{get_raspi_board, RaspiBoard};
use core::sync::atomic::{AtomicPtr, Ordering};

fn get_mimo_base() -> *mut u8 {
    const RASPI2_MIMO_BASE: *mut u8 = 0x3F000000 as *mut u8;
    const RASPI4_MIMO_BASE: *mut u8 = 0xFE000000 as *mut u8;
    const RASPIOTHER_MIMO_BASE: *mut u8 = 0x20000000 as *mut u8;

    match get_raspi_board() {
        RaspiBoard::TWO | RaspiBoard::THREE => RASPI2_MIMO_BASE,
        RaspiBoard::FOUR => RASPI4_MIMO_BASE,
        _ => RASPIOTHER_MIMO_BASE,
    }
}

pub(crate) struct Mimo {
    base: AtomicPtr<u8>,
}

impl Mimo {
    pub(crate) fn init(&self) {
        if self.base.load(Ordering::Relaxed) == core::ptr::null_mut() {
            MIMO.base.store(get_mimo_base(), Ordering::Release);
        }
    }

    pub(crate) fn base(&self) -> Option<*mut u8> {
        let base = self.base.load(Ordering::Relaxed);
        if base != core::ptr::null_mut() {
            Some(base)
        } else {
            None
        }
    }

    pub(crate) unsafe fn write<const R: ArchRegisters>(&self, data: u32) {
        core::ptr::write_volatile(self.base().unwrap().offset(R as isize) as *mut u32, data)
    }

    pub(crate) unsafe fn read<const R: ArchRegisters>(&self) -> u32 {
        core::ptr::read_volatile(self.base().unwrap().offset(R as isize) as *mut u32)
    }
}

pub(crate) static MIMO: Mimo = Mimo {
    base: AtomicPtr::new(core::ptr::null_mut()),
};
