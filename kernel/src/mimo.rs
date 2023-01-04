/// Physical devices.

pub const DRAM_BASE: usize = 0x0000_0000;
pub const DRAM_SIZE: usize = 0x3E00_0000;

pub const PERIPHERALS_BASE: usize = 0x3F00_0000;
pub const PERIPHERALS_SIZE: usize = 16 * 1024 * 1024;

pub const PERIPHERAL_IC_BASE: usize = PERIPHERALS_BASE + 0xB200;
pub const PERIPHERAL_IC_SIZE: usize = 0x24;

pub const GPIO_BASE: usize = PERIPHERALS_BASE + 0x20_0000;
pub const GPIO_SIZE: usize = 0xA0;

pub const PL011_UART_BASE: usize = PERIPHERALS_BASE + 0x20_1000;
pub const PL011_UART_SIZE: usize = 0x48;

// Local Peripheral Registers
pub const LOCAL_REGISTERS_BASE: usize = 0x4000_0000;
// Core0 timers Interrupt control register
pub const CNTP_EL0: usize = LOCAL_REGISTERS_BASE + 0x40;
// Core0 IRQ Source register
pub const CNTP_STATUS_EL0: usize = LOCAL_REGISTERS_BASE + 0x60;
pub const LOCAL_REGISTERS_SIZE: usize = 0xFC;

pub const END: usize = 0x4003_FFFF;

mod detail {
    pub trait Addressable<T> {
        fn as_ptr(&self) -> *const T;
        fn as_mut_ptr(&mut self) -> *mut T;
    }

    impl<T> Addressable<T> for u64 {
        fn as_ptr(&self) -> *const T {
            *self as *const T
        }
        fn as_mut_ptr(&mut self) -> *mut T {
            *self as *mut T
        }
    }

    impl<T> Addressable<T> for usize {
        fn as_ptr(&self) -> *const T {
            *self as *const T
        }
        fn as_mut_ptr(&mut self) -> *mut T {
            *self as *mut T
        }
    }
}

/// .
///
/// # Safety
///
/// Writes to given Memory Mapped register
pub(crate) unsafe fn write_reg<T: Sized + Copy, AddressType: detail::Addressable<T>>(
    mut addr: AddressType,
    val: T,
) {
    core::ptr::write_volatile(addr.as_mut_ptr(), val);
}

/// .
///
/// # Safety
///
/// Writes to given Memory Mapped register
pub(crate) unsafe fn read_reg<T: Sized + Copy, AddressType: detail::Addressable<T>>(
    addr: AddressType,
) -> T {
    core::ptr::read_volatile(addr.as_ptr())
}
