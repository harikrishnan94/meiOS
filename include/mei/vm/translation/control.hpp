#pragma once

#include <array>
#include <bit>
#include <numeric>

#include <ktl/access.hpp>

#include <mei/vm/address.hpp>

namespace mei::vm::translation {
// NOLINTBEGIN(*-dynamic-static-initializers, *-magic-numbers)
namespace detail {
template<std::unsigned_integral I>
constexpr auto is_pow_2(I v) noexcept -> bool {
  return (v & (v - 1)) == 0;
}

template<std::unsigned_integral I>
constexpr auto ceil_div(I a, I b) noexcept -> I {
  return a / b + (a % b ? 1 : 0);
}

template<typename T>
struct is_array_helper {
  static constexpr auto value = false;
};
template<typename T, auto N>
struct is_array_helper<std::array<T, N>> {
  static constexpr auto value = true;
};
template<typename T, auto N>
struct is_array_helper<T[N]> {  // NOLINT(*-avoid-c-arrays)
  static constexpr auto value = true;
};

// True for std::array and C-array
template<typename T>
concept is_array = is_array_helper<std::decay_t<T>>::value;

constexpr inline auto bits_per_byte = std::numeric_limits<ktl::u8>::digits;
}  // namespace detail

namespace param {
namespace detail {
template<typename Tag, std::unsigned_integral T, T Value>
struct param {
  using tag = Tag;
  static constexpr auto value = Value;
};
}  // namespace detail

template<ktl::u32 Value>
using VirtualAddressBits = detail::param<struct VirtualAddressBitsTag, ktl::u32, Value>;

template<ktl::u32 Value>
using VirtualAddressSpaceBits = detail::param<struct VirtualAddressSpaceBitsTag, ktl::u32, Value>;

template<bool Value>
using TopByteIgnore = detail::param<struct TopByteIgnoreTag, bool, Value>;

template<ktl::u32 Value>
  requires(Value != 0 && Value <= 32)
using GranuleBits = detail::param<struct GranuleBitsTag, ktl::u32, Value>;

template<ktl::u32 Value>
  requires(Value != 0 && translation::detail::is_pow_2(Value))
using GranuleSize = GranuleBits<std::bit_width(Value - 1)>;

template<ktl::u32 Value>
  requires(Value != 0 && Value <= 32)
using MaxBitsPerLevel = detail::param<struct MaxBitsPerLevelTag, ktl::u32, Value>;
}  // namespace param

// Address Translation Control: Controls the behaviour of address translation
template<
    typename VirtualAddressBits,
    typename VirtualAddressSpaceBits,
    typename TopByteIgnore,
    typename GranuleBitsOrSize,
    typename MaxBitsPerLevel>
  requires(
      requires {
        { std::same_as<typename VirtualAddressBits::tag, param::VirtualAddressBitsTag> };
        { std::same_as<typename VirtualAddressSpaceBits::tag, param::VirtualAddressSpaceBitsTag> };
        { std::same_as<typename TopByteIgnore::tag, param::TopByteIgnoreTag> };
        { std::same_as<typename GranuleBitsOrSize::tag, param::GranuleBitsTag> };
        { std::same_as<typename MaxBitsPerLevel::tag, param::MaxBitsPerLevelTag> };
      } &&
      // Support only either 32 bit or 64 bit systems
      (VirtualAddressBits::value == 32 || VirtualAddressBits::value == 64)
      // Virtual Address Space Bits must be less than Virtual Address Bits and must contain atleast
      // one unused bit excluding Top Byte -> So, total of 9 bits
      && (VirtualAddressBits::value - VirtualAddressSpaceBits::value >= 9)
      // Virtual Address Bit Count must match sum of bits for levels and at each level no more than
      // 32 bits must used
      && (MaxBitsPerLevel::value < VirtualAddressSpaceBits::value))
struct Control {
 public:
  static constexpr auto virtual_address_bits = VirtualAddressBits::value;
  static constexpr auto virtual_address_space_bits = VirtualAddressSpaceBits::value;
  static constexpr auto top_byte_ignore = TopByteIgnore::value;
  static constexpr auto granule_bits = GranuleBitsOrSize::value;
  static constexpr auto max_bits_per_level = MaxBitsPerLevel::value;
};

// Matches any address translation control like structure.
template<typename Control>
concept control_like = requires {
  { Control::virtual_address_bits } -> std::convertible_to<ktl::u32>;
  { Control::virtual_address_space_bits } -> std::convertible_to<ktl::u32>;
  { Control::top_byte_ignore } -> std::convertible_to<bool>;
  { Control::granule_bits } -> std::convertible_to<ktl::u32>;
  { Control::max_bits_per_level } -> std::convertible_to<ktl::u32>;
};

// Defined properties
template<control_like Control>
constexpr auto virtual_address_bits = Control::virtual_address_bits;
template<control_like Control>
constexpr auto virtual_address_space_bits = Control::virtual_address_space_bits;
template<control_like Control>
constexpr auto top_byte_ignore = Control::top_byte_ignore;
template<control_like Control>
constexpr auto granule_bits = Control::granule_bits;
template<control_like Control>
constexpr auto max_bits_per_level = Control::max_bits_per_level;

// Computed properties from Control

// Granule Size
template<control_like Control>
constexpr auto granule_size = ktl::u32 {1} << granule_bits<Control>;

// Unused MSB in Virtual Address
template<control_like Control>
constexpr auto unused_msb = (virtual_address_bits<Control> - virtual_address_space_bits<Control>)-(
    top_byte_ignore<Control> ? detail::bits_per_byte : 0);

// # translation levels
template<control_like Control>
constexpr ktl::u32 num_levels = detail::ceil_div(
    virtual_address_space_bits<Control> - granule_bits<Control>,
    max_bits_per_level<Control>);

// bits used to index virtual address in each level
template<control_like Control>
constexpr auto nbits_for_level = [] {
  std::array<ktl::u32, num_levels<Control>> nbits_for_level = {};
  auto rem_bits = virtual_address_space_bits<Control> - granule_bits<Control>;
  for (ktl::u32 i = 0; i < nbits_for_level.size(); i++) {
    auto cur_bits =
        rem_bits >= max_bits_per_level<Control> ? max_bits_per_level<Control> : rem_bits;
    ktl::at(nbits_for_level, i) = cur_bits;
    rem_bits -= cur_bits;
  }
  std::swap(nbits_for_level.front(), nbits_for_level.back());
  if (rem_bits != 0)
    abort_("");
  return nbits_for_level;
}();

// offset of each level inside vaddr space
template<control_like Control>
constexpr auto start_bit_for_level = [] {
  std::array<ktl::u32, num_levels<Control>> offset = {};
  ktl::at(offset, 0) = virtual_address_space_bits<Control> - ktl::at(nbits_for_level<Control>, 0);
  for (ktl::u32 i = 1; i < offset.size(); i++) {
    ktl::at(offset, i) = ktl::at(offset, i - 1) - ktl::at(nbits_for_level<Control>, i);
  }
  return offset;
}();

// Entries per level in descriptor table
template<control_like Control>
constexpr auto entries_per_level = [] {
  std::array<ktl::u32, num_levels<Control>> entries_in_level = {};
  for (ktl::u32 i = 0; i < entries_in_level.size(); i++) {
    ktl::at(entries_in_level, i) = ktl::u32 {1} << ktl::at(nbits_for_level<Control>, i);
  }
  return entries_in_level;
}();

// Output address range covered per entry
template<control_like Control>
constexpr auto out_addr_covered_per_entry = [] {
  std::array<ktl::u64, num_levels<Control>> out_addr_covered_per_entry = {};
  for (ktl::u32 i = 0; i < out_addr_covered_per_entry.size(); i++) {
    ktl::at(out_addr_covered_per_entry, i) = ktl::u64 {1}
        << ktl::at(start_bit_for_level<Control>, i);
  }
  return out_addr_covered_per_entry;
}();

template<control_like Control, ktl::u32 Level>
  requires(Level < Control::bits_in_level.size())
constexpr auto GetIndexForLevel(VirtualAddress vaddr) -> ktl::u32 {
  constexpr auto offset = ktl::at(start_bit_for_level<Control>, Level);
  constexpr auto count = ktl::at(nbits_for_level<Control>, Level);
  return Extract(vaddr, offset, count);
}

// Extracts the top MSB (excluding Top Byte if ignored).
template<control_like Control>
constexpr auto GetTopBits(VirtualAddress vaddr) noexcept -> bool {
  return Extract(vaddr, virtual_address_space_bits<Control>, unused_msb<Control>);
}

// Extracts the top byte
template<control_like Control>
constexpr auto GetTopByte(VirtualAddress vaddr) noexcept -> bool {
  return Extract(
      vaddr,
      virtual_address_bits<Control> - detail::bits_per_byte,
      detail::bits_per_byte);
}

namespace test {
using cntrl = Control<
    param::VirtualAddressBits<64>,
    param::VirtualAddressSpaceBits<48>,
    param::TopByteIgnore<true>,
    param::GranuleBits<14>,
    param::MaxBitsPerLevel<11>>;

// Verify `Control<...>` if satisfies `control_like`
static_assert(control_like<cntrl>);

static_assert(granule_size<cntrl> == 16384);
static_assert(unused_msb<cntrl> == 8);
static_assert(num_levels<cntrl> == 4);

static_assert(nbits_for_level<cntrl>[0] == 1);
static_assert(nbits_for_level<cntrl>[1] == 11);
static_assert(nbits_for_level<cntrl>[2] == 11);
static_assert(nbits_for_level<cntrl>[3] == 11);

static_assert(start_bit_for_level<cntrl>[0] == 47);
static_assert(start_bit_for_level<cntrl>[1] == 36);
static_assert(start_bit_for_level<cntrl>[2] == 25);
static_assert(start_bit_for_level<cntrl>[3] == 14);

static_assert(entries_per_level<cntrl>[0] == 2);
static_assert(entries_per_level<cntrl>[1] == 2048);
static_assert(entries_per_level<cntrl>[2] == 2048);
static_assert(entries_per_level<cntrl>[3] == 2048);

static_assert(out_addr_covered_per_entry<cntrl>[0] == 14'07'37'48'83'55'328);
static_assert(out_addr_covered_per_entry<cntrl>[1] == 68'71'94'76'736);
static_assert(out_addr_covered_per_entry<cntrl>[2] == 3'35'54'432);
static_assert(out_addr_covered_per_entry<cntrl>[3] == 16'384);
}  // namespace test
// NOLINTEND(*-dynamic-static-initializers, *-magic-numbers)
}  // namespace mei::vm::translation