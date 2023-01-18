#include "generated/uart.h"

#include "mei/print.h"
#include "mei/register/mmio.h"
#include "mei/utils/global.h"

namespace mei {
using namespace registers;
using namespace mei::registers::uart;

static constexpr usize UARTBase = 0x3F00'0000;
static constexpr usize UARTEnd = 0x48;

using PL011UART = MMIODevice<struct pl011_uart, UARTEnd>;

// NOLINTBEGIN
// Data register
using DR = MMIORegister<Data::Register, 0x000>;
// Flag register
using FR = MMIORegister<Flag::Register, 0x018>;
// Integer Baud Rate register
using IBRD = MMIORegister<IntegerBaudRate::Register, 0x024>;
// Fractional Baud Rate register
using FBRD = MMIORegister<FractionalBaudRate::Register, 0x028>;
// Line Control register
using LCR = MMIORegister<LineControl::Register, 0x02C>;
// Control register
using CR = MMIORegister<Control::Register, 0x030>;
using IMSC = MMIORegister<InterruptMaskSetClear::Register, 0x038>;
using MIS = MMIORegister<MaskedInterruptStatus::Register, 0x040>;
using ICR = MMIORegister<InterruptClear::Register, 0x044>;
// NOLINTEND

const PL011UART pl011uart{std::bit_cast<void *>(UARTBase)};

void Initialize() {
  pl011uart.Locate<CR>().Set(0);
  pl011uart.Locate<IBRD>().Set(26);
  pl011uart.Locate<FBRD>().Set(0);

  pl011uart.Locate<LCR>() |= LineControl::Register::WLEN::Value(2);
  pl011uart.Locate<IMSC>() |= InterruptMaskSetClear::Register::RXIM::SET;
  pl011uart.Locate<CR>() |= Control::Register::Enable::SET;
}

void write_byte(char byte) {
  while (Read<FR::Register::BUSY>(pl011uart.Locate<FR>()) != 0) {
  }

  pl011uart.Locate<DR>().Set(byte);
}

namespace io {
namespace dtl {
struct UARTWriter : Writer {
  auto Write(std::string_view buf) -> void override {
    for (auto c : buf) {
      write_byte(c);
    }
  }
};

static UARTWriter writer;
static bool is_uart_initialized = false;
}  // namespace dtl

auto PL011UARTWriter() -> Writer & {
  if (!dtl::is_uart_initialized) {
    Initialize();
    dtl::is_uart_initialized = true;
  }

  return dtl::writer;
}
}  // namespace io
}  // namespace mei

void operator delete(void * /*unused*/, unsigned long /*unused*/) {}
void operator delete(void * /*unused*/) {}

extern "C" void __cxa_atexit() {}
void *__dso_handle;