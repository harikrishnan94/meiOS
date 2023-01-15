#pragma once

#include <algorithm>
#include <array>
#include <concepts>

#include "mei/types.h"

#define REG(TYPE) typename TYPE::Register
#define INTT(REG) typename REG::IntType

namespace mei::registers {
namespace dtl {
template <usize Length>
struct fixed_string {
  constexpr fixed_string(const char (&str)[Length]) { std::copy_n(str, Length, value.begin()); }

  [[nodiscard]] constexpr auto data() const -> const char * { return value.data(); }

  std::array<char, Length> value = {};
};

template <typename T, typename U, typename DT = std::decay_t<T>, typename DU = std::decay_t<U>>
concept same_as = std::is_same_v<DT, DU> && std::is_same_v<DU, DT>;
}  // namespace dtl

// clang-format off
template <typename R>
concept register_t = requires(R)
// clang-format on
{
  requires std::unsigned_integral<INTT(R)>;
  { R::Name } -> dtl::same_as<const char *>;
};

template <std::unsigned_integral I, dtl::fixed_string TName>
struct Register {
  using IntType = I;
  static constexpr const char *Name = TName.data();
};
}  // namespace mei::registers
