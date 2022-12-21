use core::fmt::Write;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};
use tock_registers::{register_bitfields, register_structs};

register_structs! {
    Registers {
        // Data register
        (0x000 => dr: ReadWrite<u32, Data::Register>),
        (0x004 => _reserved1),
        // Flag register
        (0x018 => fr: ReadOnly<u32, Flag::Register>),
        (0x01C => _reserved2),
        // Integer Baud Rate register
        (0x024 => ibrd: WriteOnly<u32, IntegerBaudRate::Register>),
        // Fractional Baud Rate register
        (0x028 => fbrd: WriteOnly<u32, FractionalBaudRate::Register>),
        // Line Control register
        (0x02C => lcr: WriteOnly<u32, LineControl::Register>),
        // Control register
        (0x030 => cr: WriteOnly<u32, Control::Register>),
        (0x034 => _reserved3),
        (0x038 => imsc: WriteOnly<u32, InterruptMaskSetClear::Register>),
        (0x03C => _reserved4),
        (0x1000 => @END),
    }
}

struct Pl011Uart(&'static mut Registers);

impl Default for Pl011Uart {
    fn default() -> Self {
        unsafe {
            Self(
                (crate::gpio::UART_BASE_ADDR as *mut Registers)
                    .as_mut()
                    .unwrap(),
            )
        }
    }
}

impl Pl011Uart {
    fn init(&mut self) {
        self.0.cr.set(0);
        self.0.ibrd.set(26);
        self.0.fbrd.set(0);

        self.0
            .lcr
            .write(LineControl::FEN::SET + LineControl::WLEN.val(2));
        self.0.imsc.set(0);
        self.0
            .cr
            .write(Control::ENABLE::SET + Control::RXE::SET + Control::TXE::SET);
    }

    fn read_byte(&mut self) -> u8 {
        while self.0.fr.is_set(Flag::RXFE) {}
        self.0.dr.get() as u8
    }

    fn write_byte(&mut self, byte: u8) {
        while self.0.fr.is_set(Flag::TXFF) {}
        self.0.dr.set(byte as u32)
    }

    fn write_str(&mut self, s: &str) {
        for char in s.as_bytes() {
            self.write_byte(*char);
        }
    }
}

impl Write for Pl011Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// Like the `print!` macro in the standard library, but prints to the UART0 instance.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::uart::_print(format_args!($($arg)*)));
}

/// Like the `println!` macro in the standard library, but prints to the UART0 instance.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

lazy_static! {
    static ref UART0: spin::Mutex<Pl011Uart> = {
        let mut uart = Pl011Uart::default();
        uart.init();
        spin::Mutex::new(uart)
    };
}

/// Prints the given formatted string to the UART0 instance.
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    UART0.lock().write_fmt(args).unwrap();
}

// UART Register Fields:
register_bitfields![u32,
    // Control Register
    Control [
        /// UART enable
        ENABLE 0,
        /// Transmit enable
        TXE 8,
        /// Receive enable
        RXE 9
    ],

    // Flag Register
    Flag [
        /// Receive FIFO Empty?
        RXFE 4,
        /// Transmit FIFO Full?
        TXFF 5
    ],

    // Line Control Register
    LineControl [
        /// Enable FIFOs
        FEN OFFSET(4) NUMBITS(1) [],

        /// Word length. These bits indicate the number of data bits transmitted or received in a frame as follows
        WLEN OFFSET(5) NUMBITS(2) []
    ],

    // Data Register
    Data [
        /// Data
        DR OFFSET(0) NUMBITS(8) [],
    ],

    // Integer Baud Rate Register
    IntegerBaudRate [
        /// The integer baud rate divisor
        BAUD_DIVINT OFFSET(0) NUMBITS(16) [],
    ],

    // Fractional Baud Rate Register
    FractionalBaudRate [
        /// The fractional baud rate divisor
        BAUD_DIVFRAC OFFSET(0) NUMBITS(6) [],
    ],

    // Interrupt Mask Set/Clear Register
    InterruptMaskSetClear [
        /// All Masks
        MASK OFFSET(0) NUMBITS(11) []
    ]
];
