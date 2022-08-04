use core::panic::PanicInfo;

use crate::{exit::exit, exit::ExitCode, println};

#[panic_handler]
fn on_panic(_info: &PanicInfo) -> ! {
    println!("{}", _info);
    exit(ExitCode::Failure)
}
