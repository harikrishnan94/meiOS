#pragma once

#include <bit>
#include <concepts>

#include "types.h"

// A constexpr full-featured bitset
namespace mei::bits {
template <std::unsigned_integral I>
constexpr auto CreateMask(uint count, uint start_pos = 0) {
  return (~(I(-1) << count)) << start_pos;
}

template <std::unsigned_integral I>
constexpr auto Get(I val, uint start_pos, uint count) {
  return val & CreateMask<I>(count, start_pos);
}

template <std::unsigned_integral I>
constexpr auto Clear(I val, uint start_pos, uint count) {
  return val & ~CreateMask<I>(count, start_pos);
}

template <std::unsigned_integral I>
constexpr auto Set(I lhs, I rhs, uint start_pos, uint count) {
  return Clear(lhs, start_pos, count) | Get(rhs, start_pos, count);
}

template <std::unsigned_integral I>
constexpr auto Set(I mask, I lhs, I rhs) {
  return (lhs & ~mask) | (rhs & mask);
}

template <std::unsigned_integral I, typename... Bits>
constexpr auto CreateMask(Bits... bits) -> I {
  return ((I(1) << bits) | ...);
}

template <std::unsigned_integral I>
constexpr auto Any(I val, I mask) -> bool {
  return (val & mask) != 0;
}

template <std::unsigned_integral I>
constexpr auto All(I val, I mask) -> bool {
  return (val & mask) == mask;
}

template <std::unsigned_integral I>
constexpr auto Count(I val, I mask = I(-1)) -> uint {
  return std::popcount(val & mask);
}
}  // namespace mei::bits