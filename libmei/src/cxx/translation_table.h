#pragma once

namespace mei {
[[noreturn]] void terminate();

namespace mmu::tt {
using coro_generator_t = void *;
struct TraverseContext;
struct VMMap;

void BeginTraversal(TraverseContext &ctx);

VMMap NextItem(TraverseContext &ctx);

void EndTraversal(TraverseContext &ctx);
}  // namespace mmu::tt
}  // namespace mei