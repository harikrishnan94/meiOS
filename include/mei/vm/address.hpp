#pragma once

#include <concepts>
#include <cstdint>
#include <type_traits>

#include <ktl/bitops.hpp>

namespace mei::vm {
struct PhysicalAddress {
  uintptr_t value;
};

struct VirtualAddress {
  uintptr_t value;
};

template<typename Address>
concept address_like = std::is_aggregate_v<Address> && requires(Address a) {
  { a.value } -> std::same_as<std::add_lvalue_reference_t<uintptr_t>>;
};

static_assert(address_like<PhysicalAddress> && address_like<VirtualAddress>);

constexpr auto Extract(address_like auto addr, ktl::uint start, ktl::uint count) noexcept
    -> uintptr_t {
  return ktl::GetMaskedBits(addr.value, ktl::CreateMask<uintptr_t>(start, count)) >> start;
}
}  // namespace mei::vm