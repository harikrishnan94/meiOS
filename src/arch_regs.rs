#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[repr(u32)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ArchRegisters {
    // The offsets for reach register.
    GPIO_BASE = 0x200000,

    // Controls actuation of pull up/down to ALL GPIO pins.
    GPPUD = (ArchRegisters::GPIO_BASE as u32 + 0x94),

    // Controls actuation of pull up/down for specific GPIO pin.
    GPPUDCLK0 = (ArchRegisters::GPIO_BASE as u32 + 0x98),

    // The base address for UART.
    UART0_BASE = (ArchRegisters::GPIO_BASE as u32 + 0x1000), // for raspi4 0xFE201000, raspi2 & 3 0x3F201000, and 0x20201000 for raspi1

    // The offsets for reach register for the UART.
    UART0_RSRECR = (ArchRegisters::UART0_BASE as u32 + 0x04),
    UART0_FR = (ArchRegisters::UART0_BASE as u32 + 0x18),
    UART0_ILPR = (ArchRegisters::UART0_BASE as u32 + 0x20),
    UART0_IBRD = (ArchRegisters::UART0_BASE as u32 + 0x24),
    UART0_FBRD = (ArchRegisters::UART0_BASE as u32 + 0x28),
    UART0_LCRH = (ArchRegisters::UART0_BASE as u32 + 0x2C),
    UART0_CR = (ArchRegisters::UART0_BASE as u32 + 0x30),
    UART0_IFLS = (ArchRegisters::UART0_BASE as u32 + 0x34),
    UART0_IMSC = (ArchRegisters::UART0_BASE as u32 + 0x38),
    UART0_RIS = (ArchRegisters::UART0_BASE as u32 + 0x3C),
    UART0_MIS = (ArchRegisters::UART0_BASE as u32 + 0x40),
    UART0_ICR = (ArchRegisters::UART0_BASE as u32 + 0x44),
    UART0_DMACR = (ArchRegisters::UART0_BASE as u32 + 0x48),
    UART0_ITCR = (ArchRegisters::UART0_BASE as u32 + 0x80),
    UART0_ITIP = (ArchRegisters::UART0_BASE as u32 + 0x84),
    UART0_ITOP = (ArchRegisters::UART0_BASE as u32 + 0x88),
    UART0_TDR = (ArchRegisters::UART0_BASE as u32 + 0x8C),

    // The offsets for Mailbox registers
    MBOX_BASE = 0xB880,
    MBOX_STATUS = (ArchRegisters::MBOX_BASE as u32 + 0x18),
    MBOX_WRITE = (ArchRegisters::MBOX_BASE as u32 + 0x20),
}

impl ArchRegisters {
    pub const UART0_DR: ArchRegisters = ArchRegisters::UART0_BASE;
    pub const MBOX_READ: ArchRegisters = ArchRegisters::MBOX_BASE;
}
