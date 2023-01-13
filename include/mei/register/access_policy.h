#pragma once

#include <atomic>
#include <concepts>

#include "mei/types.h"

namespace mei::registers {
// clang-format off
template <typename P, typename I>
concept access_policy = std::unsigned_integral<I> && requires(P, const I *ro_reg, I *rw_reg, I val)
// clang-format on
{
  { P::Read(ro_reg) } -> std::same_as<I>;
  { P::Write(rw_reg, val) };
};

template <std::unsigned_integral I>
struct InMemoryAccessPolicy {
  [[nodiscard]] static constexpr auto Read(const I *mem) -> I { return *mem; }

  static constexpr void Write(I *mem, I val) { *mem = val; }
};

static_assert(access_policy<InMemoryAccessPolicy<u128>, u128>,
              "InMemoryAccessPolicy cannot be used as access_policy");

template <std::unsigned_integral I>
struct MemoryMappedAccessPolicy {
  [[nodiscard]] static auto Read(const I *mem) -> I {
    return std::atomic_ref<const I>(*mem).load(std::memory_order::relaxed);
  }

  static void Write(I *mem, I val) {
    return std::atomic_ref<I>(*mem).store(val, std::memory_order::relaxed);
  }
};

static_assert(access_policy<MemoryMappedAccessPolicy<u64>, u64>,
              "MemoryMappedAccessPolicy cannot be used as access_policy");

// SystemRegisterAccessPolicy will be generated.
}  // namespace mei::registers

using namespace mei;

// Example
struct SCTLR_EL1_AccessPolicy {
  [[nodiscard]] static auto Read(const u64 * /* mem */) -> u64 {
    u64 val;
    asm volatile("MRS %[val], SCTLR_EL1" : [val] "=r"(val));
    return val;
  }

  static void Write(u64 * /* mem */, u64 val) {
    asm volatile("MSR SCTLR_EL1, %[val]" : : [val] "r"(val));
  }
};

static_assert(mei::registers::access_policy<SCTLR_EL1_AccessPolicy, u64>,
              "SCTLR_EL1_AccessPolicy cannot be used as access_policy");
