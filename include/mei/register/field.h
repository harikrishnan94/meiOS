#pragma once

#include <algorithm>
#include <array>
#include <concepts>

#include "mei/bitops.h"
#include "mei/types.h"

namespace mei::registers {
namespace dtl {
template <size_t Length>
struct fixed_string {
  constexpr fixed_string(const char (&str)[Length]) { std::copy_n(str, Length, value.begin()); }

  [[nodiscard]] constexpr auto data() const -> const char* { return value.data(); }

  std::array<char, Length> value = {};
};

template <typename T, typename U, typename DT = std::decay_t<T>, typename DU = std::decay_t<U>>
concept same_as = std::is_same_v<DT, DU> && std::is_same_v<DU, DT>;
}  // namespace dtl

template <dtl::fixed_string TName, uint TOffset, uint TNumBits>
struct Field {
  static constexpr const char* Name = TName.data();
  static constexpr uint Offset = TOffset;
  static constexpr uint NumBits = TNumBits;
};

// clang-format off
template <typename F>
concept field = requires(F)
// clang-format on
{
  { F::Name } -> dtl::same_as<const char*>;
  { F::Offset } -> dtl::same_as<uint>;
  { F::NumBits } -> dtl::same_as<uint>;
};

template <std::unsigned_integral I, field F>
static constexpr bool is_valid_field_v = (F::NumBits + F::Offset) <= bits::Count<I>(-1);

template <std::unsigned_integral I, typename... Field>
  requires(is_valid_field_v<I, Field> && ...)
constexpr auto CreateMaskOverlapping() {
  return ((bits::CreateMask<I>(Field::NumBits, Field::Offset)) | ...);
}

template <std::unsigned_integral I, typename... Field>
static constexpr bool are_non_overlapping_v =
    bits::Count(CreateMaskOverlapping<I, Field...>()) == (Field::NumBits + ...);

template <std::unsigned_integral I, typename... Field>
  requires are_non_overlapping_v<I, Field...>
constexpr auto CreateMask() {
  return CreateMaskOverlapping<I, Field...>();
}
}  // namespace mei::registers
