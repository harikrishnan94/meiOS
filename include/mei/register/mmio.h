#pragma once

#include "mei/register/access.h"
#include "mei/types.h"

namespace mei::registers {
// clang-format off
template <typename DM>
concept mmio_register = requires(DM)
// clang-format on
{
  { DM::Offset } -> std::convertible_to<usize>;
  requires not std::is_void_v<decltype(DM::Locate(std::declval<char *>()))>;
};

template <typename DeviceTag, usize Size>
class MMIODevice {
 public:
  explicit constexpr MMIODevice(void *base_addr) : m_base(static_cast<char *>(base_addr)) {}

  template <mmio_register MR>
    requires(MR::Offset < Size)
  auto Locate() const -> auto & {
    return MR::Locate((m_base + MR::Offset));
  }

 private:
  char *m_base;
};

template <register_t R, usize O>
struct MMIORegister {
  static constexpr auto Offset = O;
  using Register = R;

  static_assert(Offset % alignof(INTT(Register)) == 0,
                "Offset must be aligned to Register field align boundary");

  static auto Locate(char *ptr) -> MemoryMappedRegister<R> & {
    return *std::bit_cast<MemoryMappedRegister<R> *>(ptr);
  }
};
}  // namespace mei::registers