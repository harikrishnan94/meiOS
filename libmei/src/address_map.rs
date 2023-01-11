use crate::address::PhysicalAddress;

/// Physical devices.

pub const DRAM_BASE: PhysicalAddress = PhysicalAddress::new(0x0000_0000);
pub const DRAM_SIZE: usize = 0x3E00_0000;
pub const DRAM_END: PhysicalAddress = DRAM_BASE + DRAM_SIZE;

pub const PERIPHERALS_BASE: PhysicalAddress = PhysicalAddress::new(0x3F00_0000);
pub const PERIPHERALS_SIZE: usize = 16 * 1024 * 1024;
pub const PERIPHERALS_END: PhysicalAddress = PERIPHERALS_BASE + PERIPHERALS_SIZE;

pub const PERIPHERAL_IC_BASE: PhysicalAddress = PERIPHERALS_BASE + 0xB200usize;
pub const PERIPHERAL_IC_SIZE: usize = 0x24;
pub const PERIPHERAL_IC_END: PhysicalAddress = PERIPHERALS_BASE + PERIPHERALS_SIZE;

pub const GPIO_BASE: PhysicalAddress = PERIPHERALS_BASE + 0x20_0000usize;
pub const GPIO_SIZE: usize = 0xA0;
pub const GPIO_END: PhysicalAddress = GPIO_BASE + GPIO_SIZE;

pub const PL011_UART_BASE: PhysicalAddress = PERIPHERALS_BASE + 0x20_1000usize;
pub const PL011_UART_SIZE: usize = 0x48;
pub const PL011_UART_END: PhysicalAddress = PL011_UART_BASE + PL011_UART_SIZE;

// Local Peripheral Registers
pub const LOCAL_REGISTERS_BASE: PhysicalAddress = PhysicalAddress::new(0x4000_0000);
// Core0 timers Interrupt control register
pub const CNTP_EL0: PhysicalAddress = LOCAL_REGISTERS_BASE + 0x40usize;
// Core0 IRQ Source register
pub const CNTP_STATUS_EL0: PhysicalAddress = LOCAL_REGISTERS_BASE + 0x60usize;
pub const LOCAL_REGISTERS_SIZE: usize = 0xFC;
pub const LOCAL_REGISTERS_END: PhysicalAddress = LOCAL_REGISTERS_BASE + LOCAL_REGISTERS_SIZE;

pub const END: PhysicalAddress = PhysicalAddress::new(0x4003_FFFF);
