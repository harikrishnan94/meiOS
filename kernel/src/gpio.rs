pub(crate) const GPIO_BASE_ADDR: u64 = 0x3f200000;
pub(crate) const UART_BASE_ADDR: u64 = GPIO_BASE_ADDR + 0x1000;

/// .
///
/// # Safety
///
/// Writes to given Memory Mapped register
pub(crate) unsafe fn write_mmio_reg<T: Sized + Copy>(addr: u64, val: T) {
    core::ptr::write_volatile(addr as *mut T, val);
}

/// .
///
/// # Safety
///
/// Writes to given Memory Mapped register
pub(crate) unsafe fn read_mmio_reg<T: Sized + Copy>(addr: u64) -> T {
    core::ptr::read_volatile(addr as *mut T)
}
