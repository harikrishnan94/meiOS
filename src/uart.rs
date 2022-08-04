use crate::arch_regs::ArchRegisters::{self, *};
use crate::board_id::{get_raspi_board, RaspiBoard};
use crate::mimo::MIMO;
use core::arch::asm;
use core::fmt::{self, Write};
use lazy_static::lazy_static;
use spin::Mutex;

pub struct Uart {}

impl Default for Uart {
    fn default() -> Self {
        let mut uart = Self {};

        uart.init();

        uart
    }
}

#[repr(align(16))]
struct MBox {
    vals: [u32; 9],
}

// A Mailbox message with set clock rate of PL011 to 3MHz tag
const MBOX: MBox = MBox {
    vals: [9 * 4, 0, 0x38002, 12, 8, 2, 3000000, 0, 0],
};

impl Uart {
    fn init(&mut self) {
        unsafe {
            // Disable UART0.
            MIMO.write::<{ UART0_CR }>(0x00000000);
            // Setup the GPIO pin 14 && 15.

            // Disable pull up/down for all GPIO pins & delay for 150 cycles.
            MIMO.write::<{ GPPUD }>(0x00000000);
            Uart::delay(150);

            // Disable pull up/down for pin 14,15 & delay for 150 cycles.
            MIMO.write::<{ GPPUDCLK0 }>((1 << 14) | (1 << 15));
            Uart::delay(150);

            // Write 0 to GPPUDCLK0 to make it take effect.
            MIMO.write::<{ GPPUDCLK0 }>(0x00000000);

            // Clear pending interrupts.
            MIMO.write::<{ UART0_ICR }>(0x7FF);

            // Set integer & fractional part of baud rate.
            // Divider = UART_CLOCK/(16 * Baud)
            // Fraction part register = (Fractional part * 64) + 0.5
            // Baud = 115200.

            // For Raspi3 and 4 the UART_CLOCK is system-clock dependent by default.
            // Set it to 3Mhz so that we can consistently set the baud rate
            let board = get_raspi_board();
            if board == RaspiBoard::THREE || board == RaspiBoard::FOUR {
                // UART_CLOCK = 30000000;
                let r = (MBOX.vals.as_ptr() as u32 & !0xF) | 8;
                // wait until we can talk to the VC
                while (MIMO.read::<{ MBOX_STATUS }>() & 0x80000000) != 0 {}
                // send our message to property channel and wait for the response
                MIMO.write::<{ MBOX_WRITE }>(r);
                while (MIMO.read::<{ MBOX_STATUS }>() & 0x40000000) != 0
                    || MIMO.read::<{ ArchRegisters::MBOX_READ }>() != r
                {}
            }

            // Divider = 3000000 / (16 * 115200) = 1.627 = ~1.
            MIMO.write::<{ UART0_IBRD }>(1);
            // Fractional part register = (.627 * 64) + 0.5 = 40.6 = ~40.
            MIMO.write::<{ UART0_FBRD }>(40);

            // Enable FIFO & 8 bit data transmission (1 stop bit, no parity).
            MIMO.write::<{ UART0_LCRH }>((1 << 4) | (1 << 5) | (1 << 6));

            // Mask all interrupts.
            MIMO.write::<{ UART0_IMSC }>(
                (1 << 1)
                    | (1 << 4)
                    | (1 << 5)
                    | (1 << 6)
                    | (1 << 7)
                    | (1 << 8)
                    | (1 << 9)
                    | (1 << 10),
            );

            // Enable UART0, receive & transfer part of UART.
            MIMO.write::<{ UART0_CR }>((1 << 0) | (1 << 8) | (1 << 9));
        }
    }

    pub fn putchar(&self, char: u8) {
        unsafe {
            // Wait for UART to become ready to transmit.
            while (MIMO.read::<{ UART0_FR }>() & (1 << 5)) != 0 {}

            MIMO.write::<{ ArchRegisters::UART0_DR }>(char as u32);
        }
    }

    pub fn getchar(&self) -> u8 {
        unsafe {
            // Wait for UART to have received something.
            while (MIMO.read::<{ UART0_FR }>() & (1 << 4)) != 0 {}

            MIMO.read::<{ ArchRegisters::UART0_DR }>() as u8
        }
    }

    pub fn write(&self, s: &str) {
        for char in s.as_bytes() {
            self.putchar(*char);
        }
    }

    #[allow(asm_sub_register)]
    fn delay(cycles: u32) {
        unsafe { asm!("1: subs {count}, {count}, #1; bne 1b\n", count = in(reg) cycles) }
    }
}

lazy_static! {
    pub static ref UART0: Mutex<Uart> = {
        let mut uart = Uart::default();
        uart.init();
        Mutex::new(uart)
    };
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s);
        Ok(())
    }
}

/// Like the `print!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::uart::_print(format_args!($($arg)*)));
}

/// Like the `println!` macro in the standard library, but prints to the VGA text buffer.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Prints the given formatted string to the VGA text buffer through the global `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    UART0.lock().write_fmt(args).unwrap();
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..10 {
        println!("test_println_many output");
    }
}
