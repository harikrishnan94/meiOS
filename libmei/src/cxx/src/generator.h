#pragma once

#include "stack_alloc.h"
#include "translation_table.h"

#include <variant>

#ifdef __GLIBCXX__
#include <coroutine>
namespace coro = std;
#else
#include <experimental/coroutine>
namespace coro = std::experimental;
#endif

namespace mei {
template <typename T> struct generator {
  struct promise_type {
    promise_type() = default;

    template <typename... Args>
    void *operator new(size_t size, std::allocator_arg_t,
                       StackAllocator<promise_type> allocator,
                       Args &&.../* args */) {
      // Round up size to next multiple of ALLOCATOR alignment
      std::size_t allocator_offset =
          (size + alignof(StackAllocator<promise_type>) - 1u) &
          ~(alignof(StackAllocator<promise_type>) - 1u);

      // Call onto allocator to allocate space for coroutine frame.
      void *ptr = allocator.allocate_bytes(
          allocator_offset + sizeof(StackAllocator<promise_type>));

      // Take a copy of the allocator (assuming noexcept copy constructor here)
      new (((char *)ptr) + allocator_offset)
          StackAllocator<promise_type>(allocator);

      return ptr;
    }

    void operator delete(void *ptr, std::size_t size) {
      std::size_t allocator_offset =
          (size + alignof(StackAllocator<promise_type>) - 1u) &
          ~(alignof(StackAllocator<promise_type>) - 1u);

      StackAllocator<promise_type> &allocator_c =
          *reinterpret_cast<StackAllocator<promise_type> *>(((char *)ptr) +
                                                            allocator_offset);

      // Move allocator to local variable first so it isn't freeing its
      // own memory from underneath itself.
      // Assuming allocator move-constructor is noexcept here.
      StackAllocator<promise_type> allocator_cp = std::move(allocator_c);

      // But don't forget to destruct allocator object in coroutine frame
      std::destroy_at(&allocator_c);

      // Finally, free the memory using the allocator.
      allocator_cp.deallocate(ptr, allocator_offset +
                                       sizeof(StackAllocator<promise_type>));
    }

    generator get_return_object() {
      return coro::coroutine_handle<promise_type>::from_promise(*this);
    }

    static coro::suspend_always initial_suspend() { return {}; }
    static coro::suspend_always final_suspend() noexcept { return {}; }

    [[noreturn]] void unhandled_exception() { terminate(); }

    void return_void() {}

    void await_transform() = delete;

    template <class U> coro::suspend_always yield_value(U &&v) {
      state.template emplace<1>(std::forward<U>(v));
      return {};
    }

    coro::suspend_always yield_value(generator &&g) {
      state.template emplace<2>(std::move(g));
      return {};
    }

  private:
    friend generator;
    struct empty {};
    std::variant<empty, T, generator> state;
  };

  bool move_next() {
    if (!coro)
      return false;

    auto &p = coro.promise();
    do {
      if (auto g = std::get_if<generator>(&p.state))
        if (g->move_next())
          return true;

      coro.resume();
      if (coro.done()) {
        coro.destroy();
        coro = {};
        return false;
      }

    } while (std::get_if<generator>(&p.state));

    return true;
  }

  T current_value() const {
    auto &p = coro.promise();
    if (auto g = std::get_if<generator>(&p.state)) {
      return g->current_value();
    } else if (auto v = std::get_if<T>(&p.state)) {
      return *v;
    } else {
      terminate();
    }
  }

  generator(const generator &) = delete;
  generator(generator &&that) : coro(that.coro) { that.coro = {}; }

  ~generator() {
    if (coro) {
      coro.destroy();
    }
  }

private:
  generator() {}
  generator(coro::coroutine_handle<promise_type> h) : coro(h) {}
  coro::coroutine_handle<promise_type> coro;
};

} // namespace mei