use core::fmt::Write;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};
use tock_registers::{register_bitfields, register_structs};

use crate::exception::ExceptionContext;
use crate::gic::{enable_irq, IRQHandler};
use crate::gic::{register_interrupt_handler, IRQNum};

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
        (0x040 => mis: ReadOnly<u32, MaskedInterruptStatus::Register>),
        (0x044 => icr: WriteOnly<u32, InterruptClear::Register>),
        (0x048 => _reserved5),
        (0x1000 => @END),
    }
}

struct Pl011Uart(&'static mut Registers);

impl Default for Pl011Uart {
    fn default() -> Self {
        unsafe {
            Self(
                (crate::mimo::PL011_UART_BASE as *mut Registers)
                    .as_mut()
                    .unwrap(),
            )
        }
    }
}

const UART_IRQ_NUM: IRQNum = 57;
const UART_IRQ_PENDING_BIT_NUM: IRQNum = 19;

impl Pl011Uart {
    fn init(&mut self) {
        self.0.cr.set(0);
        self.0.ibrd.set(26);
        self.0.fbrd.set(0);

        self.0.lcr.write(LineControl::WLEN.val(2));
        self.0.imsc.write(InterruptMaskSetClear::RXIM::SET);
        self.0
            .cr
            .write(Control::ENABLE::SET + Control::RXE::SET + Control::TXE::SET);
    }

    fn read_byte(&mut self) -> u8 {
        self.0.dr.get() as u8
    }

    fn write_byte(&mut self, byte: u8) {
        while self.0.fr.is_set(Flag::BUSY) {}
        self.0.dr.set(byte as u32)
    }

    fn write_str(&mut self, s: &str) {
        for char in s.as_bytes() {
            self.write_byte(*char);
        }
    }

    fn has_recv_irq(&self) -> bool {
        self.0.mis.is_set(MaskedInterruptStatus::RXMIS)
    }
}

lazy_static! {
    static ref IRQ_HANDLER: UARTAccessor = UARTAccessor::default();
}

struct UARTAccessor {
    uart: spin::Mutex<Pl011Uart>,
}

impl Default for UARTAccessor {
    fn default() -> Self {
        let mut uart = Pl011Uart::default();
        uart.init();

        Self {
            uart: spin::Mutex::new(uart),
        }
    }
}

impl IRQHandler for UARTAccessor {
    fn get_irq_pending_bit_num(&self) -> IRQNum {
        UART_IRQ_PENDING_BIT_NUM
    }

    fn handle(&self, _ec: &mut ExceptionContext) {
        let mut uart = self.uart.lock();
        if !uart.has_recv_irq() {
            return;
        }
        let char = uart.read_byte();
        if char == b'\r' {
            uart.write_byte(b'\n');
        } else {
            uart.write_byte(char);
        }

        // Clear Uart interrupt
        uart.0.icr.write(InterruptClear::RXIC::SET);
    }
}

/// .
///
/// # Safety
///
/// Initialize UART and Enable UART Interrupts
pub unsafe fn enable() {
    register_interrupt_handler(&*IRQ_HANDLER);
    enable_irq(UART_IRQ_NUM);
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

/// Prints the given formatted string to the UART0 instance.
#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    IRQ_HANDLER.uart.lock().write_fmt(args).unwrap();
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
        /// UART is busy transmitting data?
        BUSY 3,
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
        /// Receive interrupt mask
        RXIM 4,

        /// Transmit interrupt mask
        TXIM 5
    ],

    // Masked Interrupt Status Register
    MaskedInterruptStatus [
        /// Receive masked interrupt status
        RXMIS 4,

        /// Transmit masked interrupt status
        TXMIS 5
    ],

    // Interrupt Clear Register
    InterruptClear [
        /// Receive interrupt clear
        RXIC 4,

        /// Transmit interrupt clear
        TXIC 5
    ]
];
