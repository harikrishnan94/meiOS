#include <array>
#include <cstddef>
#include <cstdint>
#include <random>

#include "generated/exception.h"
#include "generated/mmu.h"
#include "mei/expected.h"
#include "mei/fmt/format.h"
#include "mei/kmain.h"
#include "mei/register/format.h"

extern "C" void abort(void) {
  while (true) {
  }
}

extern "C" void __assert(const char *, int, const char *) { abort(); }
extern "C" void __assert_func(const char *, int, const char *, const char *) { abort(); }
extern "C" void fmt_assert_fail(const char *file, int line, const char *message) {
  __assert(file, line, message);
}

static uint32_t MMIO_BASE;  // NOLINT(cppcoreguidelines-avoid-non-const-global-variables)

enum {
  RASPI2_MIMO_BASE = 0x3F000000,
  RASPI4_MIMO_BASE = 0xFE000000,
  RASPI_DEF_MIMO_BASE = 0x20000000,
};

// The MMIO area base address, depends on board type
static inline void mmio_init(int raspi) {
  switch (raspi) {
    case 2:
    case 3:
      MMIO_BASE = RASPI2_MIMO_BASE;
      break;  // for raspi2 & 3
    case 4:
      MMIO_BASE = RASPI4_MIMO_BASE;
      break;  // for raspi4
    default:
      MMIO_BASE = RASPI_DEF_MIMO_BASE;
      break;  // for raspi1, raspi zero etc.
  }
}

// Memory-Mapped I/O output
static inline void mmio_write(uint32_t reg, uint32_t data) {
  *(volatile uint32_t *)(uintptr_t)(MMIO_BASE + reg) = data;  // NOLINT(performance-no-int-to-ptr)
}

// Memory-Mapped I/O input
static inline uint32_t mmio_read(uint32_t reg) {
  return *(volatile uint32_t *)(uintptr_t)(MMIO_BASE + reg);  // NOLINT(performance-no-int-to-ptr)
}

// Loop <delay> times in a way that the compiler won't optimize away
static inline void delay(int32_t count) {
  // NOLINTNEXTLINE(hicpp-no-assembler)
  asm volatile("__delay_%=: subs %[count], %[count], #1; bne __delay_%=\n"
               : "=r"(count)
               : [count] "0"(count)
               : "cc");
}

enum {
  // The offsets for reach register.
  GPIO_BASE = 0x200000,

  // Controls actuation of pull up/down to ALL GPIO pins.
  GPPUD = (GPIO_BASE + 0x94),

  // Controls actuation of pull up/down for specific GPIO pin.
  GPPUDCLK0 = (GPIO_BASE + 0x98),

  // The base address for UART.
  UART0_BASE = (GPIO_BASE +
                0x1000),  // for raspi4 0xFE201000, raspi2 & 3 0x3F201000, and 0x20201000 for raspi1

  // The offsets for reach register for the UART.
  UART0_DR = (UART0_BASE + 0x00),
  UART0_RSRECR = (UART0_BASE + 0x04),
  UART0_FR = (UART0_BASE + 0x18),
  UART0_ILPR = (UART0_BASE + 0x20),
  UART0_IBRD = (UART0_BASE + 0x24),
  UART0_FBRD = (UART0_BASE + 0x28),
  UART0_LCRH = (UART0_BASE + 0x2C),
  UART0_CR = (UART0_BASE + 0x30),
  UART0_IFLS = (UART0_BASE + 0x34),
  UART0_IMSC = (UART0_BASE + 0x38),
  UART0_RIS = (UART0_BASE + 0x3C),
  UART0_MIS = (UART0_BASE + 0x40),
  UART0_ICR = (UART0_BASE + 0x44),
  UART0_DMACR = (UART0_BASE + 0x48),
  UART0_ITCR = (UART0_BASE + 0x80),
  UART0_ITIP = (UART0_BASE + 0x84),
  UART0_ITOP = (UART0_BASE + 0x88),
  UART0_TDR = (UART0_BASE + 0x8C),

  // The offsets for Mailbox registers
  MBOX_BASE = 0xB880,
  MBOX_READ = (MBOX_BASE + 0x00),
  MBOX_STATUS = (MBOX_BASE + 0x18),
  MBOX_WRITE = (MBOX_BASE + 0x20)
};

// A Mailbox message with set clock rate of PL011 to 3MHz tag
volatile unsigned int __attribute__((aligned(16)))
mbox[9] = {9 * 4, 0, 0x38002, 12, 8, 2, 3000000, 0, 0};

void uart_init(int raspi) {
  mmio_init(raspi);

  // Disable UART0.
  mmio_write(UART0_CR, 0x00000000);
  // Setup the GPIO pin 14 && 15.

  // Disable pull up/down for all GPIO pins & delay for 150 cycles.
  mmio_write(GPPUD, 0x00000000);
  delay(150);

  // Disable pull up/down for pin 14,15 & delay for 150 cycles.
  mmio_write(GPPUDCLK0, (1 << 14) | (1 << 15));
  delay(150);

  // Write 0 to GPPUDCLK0 to make it take effect.
  mmio_write(GPPUDCLK0, 0x00000000);

  // Clear pending interrupts.
  mmio_write(UART0_ICR, 0x7FF);

  // Set integer & fractional part of baud rate.
  // Divider = UART_CLOCK/(16 * Baud)
  // Fraction part register = (Fractional part * 64) + 0.5
  // Baud = 115200.

  // For Raspi3 and 4 the UART_CLOCK is system-clock dependent by default.
  // Set it to 3Mhz so that we can consistently set the baud rate
  if (raspi >= 3) {
    // UART_CLOCK = 30000000;
    unsigned int r = (((unsigned int)(uintptr_t)(&mbox) & ~0xF) | 8);
    // wait until we can talk to the VC
    while (mmio_read(MBOX_STATUS) & 0x80000000) {
    }
    // send our message to property channel and wait for the response
    mmio_write(MBOX_WRITE, r);
    while ((mmio_read(MBOX_STATUS) & 0x40000000) || mmio_read(MBOX_READ) != r) {
    }
  }

  // Divider = 3000000 / (16 * 115200) = 1.627 = ~1.
  mmio_write(UART0_IBRD, 1);
  // Fractional part register = (.627 * 64) + 0.5 = 40.6 = ~40.
  mmio_write(UART0_FBRD, 50);

  // Enable FIFO & 8 bit data transmission (1 stop bit, no parity).
  mmio_write(UART0_LCRH, (1 << 4) | (1 << 5) | (1 << 6));

  // Mask all interrupts.
  mmio_write(
      UART0_IMSC,
      (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10));

  // Enable UART0, receive & transfer part of UART.
  mmio_write(UART0_CR, (1 << 0) | (1 << 8) | (1 << 9));
}

void uart_putc(unsigned char c) {
  // Wait for UART to become ready to transmit.
  while (mmio_read(UART0_FR) & (1 << 5)) {
  }
  mmio_write(UART0_DR, c);
}

unsigned char uart_getc() {
  // Wait for UART to have received something.
  while (mmio_read(UART0_FR) & (1 << 4)) {
  }
  return mmio_read(UART0_DR);
}

void uart_puts(const char *str) {
  for (size_t i = 0; str[i] != '\0'; i++) uart_putc((unsigned char)str[i]);
}

using namespace mei::registers;

template <typename... Args>
void puts(fmt::format_string<Args...> fmt, Args &&...args) {
  char str[128] = {};

  fmt::format_to(&str[0], fmt, std::forward<Args>(args)...);
  uart_puts(&str[0]);
}

enum class error { div_by_zero, not_even };

constexpr auto get_val() -> mei::expected<mei::u64, error> {
  using STAGE1_PAGE_DESCRIPTOR = mmu::STAGE1_PAGE_DESCRIPTOR::Register;

  using PageDesc = InMemoryRegister<STAGE1_PAGE_DESCRIPTOR>;

  PageDesc desc{0};
  desc.Set(100);

  desc |= STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB::Value(0xFFFF) +
          STAGE1_PAGE_DESCRIPTOR::PXN::True - RM<STAGE1_PAGE_DESCRIPTOR::PXN>;

  return Read<STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB>(desc) |
         MaskFor<STAGE1_PAGE_DESCRIPTOR::PXN>;
}

constinit const auto Val = get_val();

static auto div_exp(int a, int b) -> tl::expected<int, error> {
  if (b == 0) Err(error::div_by_zero);
  return a / b;
}

static auto div_if_even(int a, int b) -> tl::expected<int, error> {
  if (a % 2 != 0) Err(error::not_even);
  return Try(div_exp(a, b));
}

/* Use C linkage for MEI_MAIN. */
extern "C" void MEI_MAIN() {
  uart_init(3);

  using STAGE1_PAGE_DESCRIPTOR = mmu::STAGE1_PAGE_DESCRIPTOR::Register;

  using PageDesc = InMemoryRegister<STAGE1_PAGE_DESCRIPTOR>;

  PageDesc desc{0};
  desc |= STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_4KiB::Value(0xFFFF);
  puts("{:X}\n", desc);

  const auto &CurrentEL = mei::registers::exception::CurrentEL::CurrentEL;
  // using CurrentELReg = exception::CurrentEL::Register;

  puts("{}\n", CurrentEL);
  puts("After set = 0x{:X}\n", Val.value());
}
