#pragma once

#include <concepts>

#include <ktl/int.hpp>

namespace mei::utils {
template<std::integral Int>
constexpr auto AlignUp(Int value, std::make_unsigned_t<Int> alignment) noexcept -> Int {
  using UInt = std::make_unsigned_t<Int>;
  return (static_cast<UInt>(value) + UInt {alignment - 1}) & ~UInt {alignment - 1};
}

template<std::integral Int>
constexpr auto AlignUpOffset(Int value, std::make_unsigned_t<Int> alignment) noexcept -> Int {
  return AlignUp(value, alignment) - value;
}

template<std::integral Int>
constexpr auto AlignDown(Int value, std::make_unsigned_t<Int> alignment) noexcept -> Int {
  using UInt = std::make_unsigned_t<Int>;
  return static_cast<UInt>(value) & ~UInt {alignment - 1};
}

template<std::integral Int>
constexpr auto AlignDownOffset(Int value, std::make_unsigned_t<Int> alignment) noexcept -> Int {
  return value - AlignDown(value, alignment);
}

template<std::integral Int>
constexpr inline auto IsAligned(Int value, std::make_unsigned_t<Int> alignment) noexcept -> bool {
  using UInt = std::make_unsigned_t<Int>;
  return (static_cast<UInt>(value) & UInt {alignment - 1}) == 0;
}
}  // namespace mei::utils