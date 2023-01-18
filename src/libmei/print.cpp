#include "mei/print.h"

namespace mei::io {
namespace dtl {
static Writer *stdout_writer = nullptr;
static Writer *stderr_writer = nullptr;

auto get_stdout() -> Writer * { return stdout_writer; }
auto get_stderr() -> Writer * { return stderr_writer; }
}  // namespace dtl

void SetStdOut(Writer &writer) { dtl::stdout_writer = &writer; }

void SetStdErr(Writer &writer) { dtl::stderr_writer = &writer; }
}  // namespace mei::io