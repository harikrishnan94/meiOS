#pragma once

#include <cstddef>
#include <cstdint>

namespace rust {
inline namespace cxxbridge1 {
template <typename Signature>
class Fn;
}
}  // namespace rust

namespace mei::vm::slab {
struct Layout;
enum class AllocError : uint8_t;
struct AllocResult;
struct UpstreamAllocator;

auto Init(uint8_t* phy_start,
          uint8_t* phy_end,
          rust::Fn<UpstreamAllocator(uint8_t* size, uint8_t* align)> initalize_upstream,
          rust::Fn<void(UpstreamAllocator, size_t size)> alloc_page,
          rust::Fn<void(UpstreamAllocator, uint8_t* ptr, size_t size)> free_page) -> bool;

auto Alloc(const Layout& layout) -> AllocResult;

auto Free(uint8_t*, const Layout& layout) -> AllocResult;

auto GetUpstreamAllocator() -> UpstreamAllocator;
}  // namespace mei::vm::slab