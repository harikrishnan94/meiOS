use core::panic::PanicInfo;
use qemu_exit::QEMUExit;

const EXIT_PANIC: u32 = 2;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(EXIT_PANIC)
}

#[cfg(target_arch = "aarch64")]
static QEMU_EXIT_HANDLE: qemu_exit::aarch64::AArch64 = qemu_exit::AArch64::new();

pub fn exit(exit_code: u32) -> ! {
    QEMU_EXIT_HANDLE.exit(exit_code)
}

pub fn exit_success() -> ! {
    QEMU_EXIT_HANDLE.exit_success()
}

pub fn exit_failure() -> ! {
    QEMU_EXIT_HANDLE.exit_failure()
}
