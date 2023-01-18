#pragma once

#include <experimental/coroutine>
#include <variant>

namespace coro = std::experimental;

namespace mei {
template <class T> struct generator {
  struct promise_type {
    generator get_return_object() {
      return coro::coroutine_handle<promise_type>::from_promise(*this);
    }

    static coro::suspend_always initial_suspend() { return {}; }
    static coro::suspend_always final_suspend() noexcept { return {}; }

    [[noreturn]] void unhandled_exception() { std::terminate(); }

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
      std::terminate();
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