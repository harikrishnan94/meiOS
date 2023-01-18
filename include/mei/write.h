#pragma once

#include <etl/vector.h>

#include "fmt/format.h"
#include "mei/types.h"

namespace mei::io {
// clang-format off
template <typename W>
concept writer = requires(W &w)
// clang-format on
{
  { w.Write(std::declval<std::string_view>()) } -> std::same_as<void>;
};

template <usize BF>
struct BufferSizeT {
  static constexpr auto Size = BF;
};

template <usize BF>
static constexpr BufferSizeT<BF> BufferSize = {};

template <writer W, usize BufferSize>
struct BufferedWriter {
 public:
  using value_type = char;

  explicit BufferedWriter(W &w) : m_writer(&w) {}

  void push_back(char c) {
    if (m_buf.full()) [[unlikely]] {
      Flush();
    }

    m_buf.push_back(c);
  }

  void Flush() {
    m_writer->Write(std::string_view{m_buf.begin(), m_buf.end()});
    m_buf.clear();
  }

  void Write(const char *start, const char *end) {
    while (start < end) {
      push_back(*start++);
    }
  }

 private:
  W *m_writer;
  etl::vector<char, BufferSize> m_buf;
};

template <usize BufferSize = 64>
constexpr auto Buffered(writer auto &w) {
  return BufferedWriter<std::decay_t<decltype(w)>, BufferSize>{w};
}

template <usize BufferSize, typename... Args>
void write(writer auto &writer, fmt::format_string<Args...> fmt, Args &&...args) {
  auto buf_writer = Buffered(writer);
  fmt::format_to(std::back_inserter(buf_writer), fmt, std::forward<Args>(args)...);
}
}  // namespace mei::io