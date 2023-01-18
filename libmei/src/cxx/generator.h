#pragma once

#include <memory>
#include <variant>

#include "stack_alloc.h"
#include "translation_table.h"

#ifdef __GLIBCXX__
#include <coroutine>
namespace coro = std;
#else
#include <experimental/coroutine>
namespace coro = std::experimental;
#endif

namespace mei {
template <typename T>
struct generator {
  struct promise_type {
    promise_type() = default;

    using Allocator = StackAllocator<promise_type>;

    template <typename... Args>
    void *operator new(size_t size, std::allocator_arg_t, Allocator allocator,
                       Args &&.../* args */) {
      // Round up size to next multiple of Allocator alignment
      std::size_t allocator_offset = alignas_allocator(size);

      // Call onto allocator to allocate space for coroutine frame.
      void *ptr =
          allocator.allocate_bytes(allocator_offset + sizeof(Allocator));

      // Take a copy of the allocator (assuming noexcept copy constructor here)
      std::construct_at(reinterpret_cast<Allocator *>(static_cast<char *>(ptr) +
                                                      allocator_offset),
                        std::move(allocator));

      return ptr;
    }

    void operator delete(void *ptr, std::size_t size) {
      std::size_t allocator_offset = alignas_allocator(size);

      Allocator &allocator =
          *reinterpret_cast<Allocator *>(((char *)ptr) + allocator_offset);

      // Move allocator to local variable first so it isn't freeing its
      // own memory from underneath itself.
      // Assuming allocator move-constructor is noexcept here.
      Allocator allocator_cp = std::move(allocator);

      // But don't forget to destruct allocator object in coroutine frame
      std::destroy_at(&allocator);

      // Finally, free the memory using the allocator.
      allocator_cp.deallocate_bytes(ptr, allocator_offset + sizeof(Allocator));
    }

    generator get_return_object() {
      return coro::coroutine_handle<promise_type>::from_promise(*this);
    }

    static coro::suspend_always initial_suspend() { return {}; }
    static coro::suspend_always final_suspend() noexcept { return {}; }

    [[noreturn]] void unhandled_exception() { terminate(); }

    void return_void() {}

    void await_transform() = delete;

    template <typename U = T,
              std::enable_if_t<!std::is_rvalue_reference<U>::value, int> = 0>
    coro::suspend_always yield_value(
        std::remove_reference_t<T> &value) noexcept {
      m_value = std::addressof(value);
      return {};
    }

    coro::suspend_always yield_value(
        std::remove_reference_t<T> &&value) noexcept {
      m_value = std::addressof(value);
      return {};
    }

   private:
    friend generator;

    static constexpr size_t alignas_allocator(size_t size) {
      return (size + alignof(Allocator) - 1u) & ~(alignof(Allocator) - 1u);
    }

    T *m_value;
  };

  operator bool() {
    coro.resume();
    if (coro.done()) {
      coro.destroy();
      coro = {};
      return false;
    }
    return true;
  }

  T operator()() const {
    auto &p = coro.promise();
    return std::move(*p.m_value);
  }

  generator(const generator &) = delete;
  generator &operator=(const generator &) = delete;

  generator(generator &&that) : coro(std::exchange(that.coro, {})) {}
  generator &operator=(generator &&that) {
    std::swap(that.coro, coro);
    return *this;
  }

  ~generator() {
    if (coro) coro.destroy();
  }

 private:
  generator() {}
  generator(coro::coroutine_handle<promise_type> h) : coro(h) {}
  coro::coroutine_handle<promise_type> coro;
};

}  // namespace mei