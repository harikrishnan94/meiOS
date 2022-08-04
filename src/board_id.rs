use atomic::{Atomic, Ordering};
use core::arch::asm;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum RaspiBoard {
    ONE,
    TWO,
    THREE,
    FOUR,
    OTHER,
    UNKNOWN,
}

static RASPI_BOARD: Atomic<RaspiBoard> = Atomic::new(RaspiBoard::UNKNOWN);

#[allow(asm_sub_register)]
pub(crate) fn get_raspi_board() -> RaspiBoard {
    let mut board = RASPI_BOARD.load(Ordering::Relaxed);
    if board == RaspiBoard::UNKNOWN {
        let mut reg: u32;
        unsafe {
            asm!(
                "mrs {0}, midr_el1",
                out(reg) reg,
            );
        }

        board = match (reg >> 4) & 0xFFF {
            0xB76 => RaspiBoard::ONE,
            0xC07 => RaspiBoard::TWO,
            0xD03 => RaspiBoard::THREE,
            0xD08 => RaspiBoard::FOUR,
            _ => RaspiBoard::OTHER,
        };

        RASPI_BOARD.store(board, Ordering::Release);
    }

    board
}
