use atomic::{Atomic, Ordering};
use core::arch::asm;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum RaspiBoard {
    ONE,
    TWO,
    THREE,
    FOUR,
    OTHER,
}

static RASPI_BOARD: Atomic<Option<RaspiBoard>> = Atomic::new(None);

#[allow(asm_sub_register)]
pub fn get_raspi_board() -> RaspiBoard {
    if let Some(board) = RASPI_BOARD.load(Ordering::Relaxed) {
        board
    } else {
        let mut reg: u32;
        unsafe {
            asm!(
                "mrs {0}, midr_el1",
                out(reg) reg,
            );
        }

        let board = match (reg >> 4) & 0xFFF {
            0xB76 => RaspiBoard::ONE,
            0xC07 => RaspiBoard::TWO,
            0xD03 => RaspiBoard::THREE,
            0xD08 => RaspiBoard::FOUR,
            _ => RaspiBoard::OTHER,
        };

        RASPI_BOARD.store(Some(board), Ordering::Release);
        get_raspi_board()
    }
}

#[test_case]
fn test_raspi_board() {
    assert_eq!(get_raspi_board(), RaspiBoard::THREE);
}
