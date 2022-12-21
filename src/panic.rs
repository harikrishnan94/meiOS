use core::panic::PanicInfo;

const EXIT_PANIC: u32 = 2;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    crate::exit(EXIT_PANIC)
}
