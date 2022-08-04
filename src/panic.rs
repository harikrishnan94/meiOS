use core::panic::PanicInfo;

extern "C" {
    fn _exit(code: i32) -> !;
}

#[panic_handler]
fn on_panic(_info: &PanicInfo) -> ! {
    unsafe { _exit(-1) }
}
