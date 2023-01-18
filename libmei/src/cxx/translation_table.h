#pragma once

#include "libmei/src/mmu/translation_table.rs.h"

#include <cinttypes>
#include <utility>

namespace mei::mmu::tt {
void BeginTraversal(TraverseContext &ctx);

VMMap NextItem(TraverseContext &ctx);

void EndTraversal(TraverseContext &ctx);
} // namespace mei::mmu::tt

namespace mei {
[[noreturn]] inline void terminate() {
  mmu::tt::terminate();
  __builtin_unreachable();
}
} // namespace mei