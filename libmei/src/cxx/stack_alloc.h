#pragma once

#include <array>

namespace mei {
static constexpr auto CoroMemSize = 2048;

template <size_t S>
struct RawStackAllocator {
  static constexpr auto Size = S;

  size_t used;
  std::array<char, Size - sizeof(size_t)> mem;
};

using DefaultStackAllocator = RawStackAllocator<CoroMemSize>;

template <typename T>
class StackAllocator {
 public:
  using value_type = T;

  StackAllocator(DefaultStackAllocator *alloc) : m_alloc(alloc) {}

  template <typename U>
  constexpr StackAllocator(const StackAllocator<U> &o) noexcept
      : m_alloc(o.get_backing_mem()) {}

  DefaultStackAllocator *get_backing_mem() const { return m_alloc; }

  [[nodiscard]] T *allocate(std::size_t n) {
    return static_cast<T *>(allocate_bytes(n * sizeof(T)));
  }

  [[nodiscard]] void *allocate_bytes(std::size_t bytes) {
    if (m_alloc->used + bytes > DefaultStackAllocator::Size) return nullptr;

    auto ptr = static_cast<void *>(m_alloc->mem.data() + m_alloc->used);
    m_alloc->used += bytes;

    return ptr;
  }

  void deallocate(void * /* p */, std::size_t n) noexcept {
    deallocate_bytes(n * sizeof(T));
  }

  void deallocate_bytes(void * /* p */, std::size_t bytes) noexcept {
    m_alloc->used -= bytes;
  }

 private:
  DefaultStackAllocator *m_alloc;
};
}  // namespace mei