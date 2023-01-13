#pragma once

#include <cstddef>
#include <cstdint>
#include <type_traits>

namespace mei {
// Rust like integer types
using u8 = std::uint8_t;
using u16 = std::uint16_t;
using u32 = std::uint32_t;
using u64 = std::uint64_t;
using u128 = __uint128_t;
using usize = std::size_t;
using uint = unsigned;

using i8 = std::make_signed_t<u8>;
using i16 = std::make_signed_t<u16>;
using i32 = std::make_signed_t<u32>;
using i64 = std::make_signed_t<u64>;
using i128 = std::make_signed_t<u128>;
using isize = std::make_signed_t<usize>;
}  // namespace mei