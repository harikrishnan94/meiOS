#pragma once

#include <bit>
#include <concepts>

namespace mei::bits {
using uint = unsigned;

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
} // namespace mei::bits