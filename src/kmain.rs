use mei::board_id::{get_raspi_board, RaspiBoard};
use mei::exit::ExitCode;
use mei::println;

pub(crate) extern "C" fn kmain() -> ExitCode {
    match get_raspi_board() {
        RaspiBoard::ONE => println!("Welcome to Raspberry PI 1"),
        RaspiBoard::TWO => println!("Welcome to Raspberry PI 2"),
        RaspiBoard::THREE => println!("Welcome to Raspberry PI 3"),
        RaspiBoard::FOUR => println!("Welcome to Raspberry PI 4"),
        _ => println!("Welcome to Raspberry PI XXX"),
    }

    ExitCode::Success
}
