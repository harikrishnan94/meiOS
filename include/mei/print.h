#pragma once

#include <optional>

#include "errors.h"
#include "expected.h"
#include "write.h"

namespace mei::io {
struct Writer {
  virtual ~Writer() = default;

  virtual auto Write(std::string_view buf) -> void;
  virtual auto HasError() -> std::optional<Error> { return {}; }
};

namespace dtl {
auto get_stdout() -> Writer *;
auto get_stderr() -> Writer *;

static constexpr auto PrintBufferSize = 64;
}  // namespace dtl

void SetStdOut(Writer &writer);

void SetStdErr(Writer &writer);

auto PL011UARTWriter() -> Writer &;

template <typename... Args>
auto Stdout(fmt::format_string<Args...> fmt, Args &&...args) -> expected<void, Error> {
  auto *writer = dtl::get_stdout();
  if (writer == nullptr) return make_unexpected(Error::StdOutNotSet);

  write<dtl::PrintBufferSize>(*writer, fmt, std::forward<Args>(args)...);

  if (auto err = writer->HasError()) return make_unexpected(*err);

  return {};
}

template <typename... Args>
auto Stderr(fmt::format_string<Args...> fmt, Args &&...args) -> expected<void, Error> {
  auto *writer = dtl::get_stderr();
  if (writer == nullptr) return make_unexpected(Error::StdErrNotSet);

  write<dtl::PrintBufferSize>(*writer, fmt, std::forward<Args>(args)...);

  if (auto err = writer->HasError()) return make_unexpected(*err);

  return {};
}
}  // namespace mei::io