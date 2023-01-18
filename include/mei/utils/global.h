#pragma once

#include <array>
#include <bit>
#include <concepts>

namespace mei::utils {
template <std::destructible T>
class Global {
 public:
  // Global();
  explicit Global(std::invocable<T *> auto initializer) { initializer(mem.data()); }

  Global() : Global([](auto *ptr) { new (ptr) T; }) {}

  Global(const Global &) = delete;
  Global(Global &&) = delete;
  auto operator=(const Global &) -> Global & = delete;
  auto operator=(Global &&) -> Global & = delete;

  auto Get() const -> T & { return *std::bit_cast<T *>(mem.data()); }

  auto operator*() const -> T & { return Get(); }
  auto operator->() const -> T * { return &Get(); }

 private:
  alignas(T) std::array<char, sizeof(T)> mem;
};
}  // namespace mei::utils