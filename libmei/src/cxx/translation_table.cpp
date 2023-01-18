#include <algorithm>
#include <cinttypes>

#include "bits.h"
#include "generator.h"
#include "libmei/src/mmu/translation_table.rs.h"

namespace mei {
namespace mmu::tt {
using u32 = uint32_t;
using u64 = uint64_t;
using reg = u64;
using Level = u32;

template <typename To, typename From>
static inline auto bit_cast(From from) -> To {
  union {
    From from;
    To to;
  } data;
  data.from = from;
  return data.to;
}

enum : u64 {
  ONE_GIB = 1024 * 1024 * 1024,
  TWO_MIB = 2 * 1024 * 1024,
  FOUR_KIB = 4 * 1024,
};

enum Translation : u64 {
  NUM_LEVELS = 4,
  VIRTUAL_ADDRESS_IGNORE_MSB = 16,
  VIRTUAL_ADDRESS_NBITS =
      std::countr_zero(VirtualAddress{0}.val) - VIRTUAL_ADDRESS_IGNORE_MSB,
  VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS = std::countr_one(FOUR_KIB - 1),
  VIRTUAL_ADDRESS_LEVEL_IDX_BITS = 9,
  NUM_TABLE_DESC_ENTRIES = (1ULL << VIRTUAL_ADDRESS_LEVEL_IDX_BITS)
};

enum DescriptorBits : u32 {
  DESCRIPTOR_VALID_OFFSET = 0,
  DESCRIPTOR_TYPE_OFFSET = 1,

  OUTPUT_ADDR_512GIB_NBITS = VIRTUAL_ADDRESS_LEVEL_IDX_BITS,
  OUTPUT_ADDR_512GIB_OFFSET = VIRTUAL_ADDRESS_NBITS - OUTPUT_ADDR_512GIB_NBITS,

  OUTPUT_ADDR_1GIB_NBITS =
      OUTPUT_ADDR_512GIB_NBITS + VIRTUAL_ADDRESS_LEVEL_IDX_BITS,
  OUTPUT_ADDR_1GIB_OFFSET = VIRTUAL_ADDRESS_NBITS - OUTPUT_ADDR_1GIB_NBITS,

  OUTPUT_ADDR_2MIB_NBITS =
      OUTPUT_ADDR_1GIB_NBITS + VIRTUAL_ADDRESS_LEVEL_IDX_BITS,
  OUTPUT_ADDR_2MIB_OFFSET = VIRTUAL_ADDRESS_NBITS - OUTPUT_ADDR_2MIB_NBITS,

  OUTPUT_ADDR_4KIB_NBITS =
      OUTPUT_ADDR_2MIB_NBITS + VIRTUAL_ADDRESS_LEVEL_IDX_BITS,
  OUTPUT_ADDR_4KIB_OFFSET = VIRTUAL_ADDRESS_NBITS - OUTPUT_ADDR_4KIB_NBITS,

  DESCRIPTOR_ENTRY_SIZE = sizeof(reg),
  NEXT_LEVEL_DESC_OFFSET = std::countr_zero(
      (1 << VIRTUAL_ADDRESS_LEVEL_IDX_BITS) * DESCRIPTOR_ENTRY_SIZE),
  NEXT_LEVEL_DESC_NBITS = VIRTUAL_ADDRESS_NBITS - NEXT_LEVEL_DESC_OFFSET,
};

enum class DescriptorType { INVALID = 0, TABLE = 1, BLOCK = 2, PAGE = 3 };

static constexpr u64 VA_SPACING_PER_ENTRY[] = {
    1ULL << OUTPUT_ADDR_512GIB_OFFSET, 1ULL << OUTPUT_ADDR_1GIB_OFFSET,
    1ULL << OUTPUT_ADDR_2MIB_OFFSET, 1ULL << OUTPUT_ADDR_4KIB_OFFSET};

static constexpr u64 OUTPUT_ADDRESS_OFFSET_ON_LEVEL[] = {
    OUTPUT_ADDR_512GIB_OFFSET, OUTPUT_ADDR_1GIB_OFFSET, OUTPUT_ADDR_2MIB_OFFSET,
    OUTPUT_ADDR_4KIB_OFFSET};
static constexpr u64 OUTPUT_ADDRESS_NBITS_ON_LEVEL[] = {
    OUTPUT_ADDR_512GIB_NBITS, OUTPUT_ADDR_1GIB_NBITS, OUTPUT_ADDR_2MIB_NBITS,
    OUTPUT_ADDR_4KIB_NBITS};

struct DescriptorTable {
  std::array<reg, NUM_TABLE_DESC_ENTRIES> descs;
};

static constexpr size_t get_index_for_level(VirtualAddress vaddr, Level lvl) {
  auto offset = VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS +
                (NUM_LEVELS - 1 - lvl) * VIRTUAL_ADDRESS_LEVEL_IDX_BITS;
  return bits::Get(vaddr.val, offset, VIRTUAL_ADDRESS_LEVEL_IDX_BITS) >> offset;
}

static constexpr auto parse_descriptor(reg desc, Level level)
    -> DescriptorType {
  if (!bits::Get(desc, DESCRIPTOR_VALID_OFFSET, 1))
    return DescriptorType::INVALID;

  if (bits::Get(desc, DESCRIPTOR_TYPE_OFFSET, 1)) {
    // Table or Page
    return level == NUM_LEVELS - 1 ? DescriptorType::PAGE
                                   : DescriptorType::TABLE;
  } else {
    // Block
    return DescriptorType::BLOCK;
  }
}

static auto create_vm_map(const reg &desc, Level lvl, VirtualAddress vaddr)
    -> VMMap {
  auto offset = OUTPUT_ADDRESS_OFFSET_ON_LEVEL[lvl];
  auto output_address =
      bits::Get(desc, offset, OUTPUT_ADDRESS_NBITS_ON_LEVEL[lvl]);
  auto block_len = VA_SPACING_PER_ENTRY[lvl];

  vaddr.val = bits::Clear(vaddr.val, 0, offset);

  return {{output_address}, block_len, vaddr, bit_cast<reg>(&desc)};
}

static constexpr void free_empty_descs(const DescriptorTable &root,
                                       TraverseContext &ctx) {
  if (ctx.free_empty_descs) {
    bool is_empty =
        std::all_of(root.descs.begin(), root.descs.end(), [](auto desc) {
          return bits::Get(desc, DESCRIPTOR_VALID_OFFSET, 1) == 0;
        });

    if (is_empty) {
      ctx.empty_descs[ctx.num_empty_descs] = bit_cast<u64>(&root);
      ctx.num_empty_descs++;
    }
  }
}

using yield_t = generator<VMMap>;

static auto Traverse(std::allocator_arg_t,
                     StackAllocator<yield_t::promise_type> alloc,
                     TraverseContext &ctx, const DescriptorTable &root,
                     VirtualAddress vaddr, Level lvl) -> yield_t {
  if (lvl == NUM_LEVELS) {
    ctx.has_error = true;
    co_return;
  }

  for (auto ind = get_index_for_level(vaddr, lvl);
       ind < NUM_TABLE_DESC_ENTRIES && vaddr.val < ctx.va_end.val; ind++) {
    const auto &desc = root.descs[ind];
    auto type = parse_descriptor(desc, lvl);

    switch (type) {
      case DescriptorType::INVALID:
        break;

      case DescriptorType::TABLE: {
        auto child =
            bits::Get(desc, NEXT_LEVEL_DESC_OFFSET, NEXT_LEVEL_DESC_NBITS);

        // Descend down.
        auto gen =
            Traverse(std::allocator_arg, alloc, ctx,
                     *bit_cast<const DescriptorTable *>(child), vaddr, lvl + 1);
        while (gen) {
          co_yield gen();
        }
        break;
      }

      case DescriptorType::BLOCK:
        if (lvl == 0 || lvl == 3) {
          ctx.has_error = true;
          co_return;
        }

        co_yield create_vm_map(desc, lvl, vaddr);
        break;

      case DescriptorType::PAGE:
        if (lvl != 3) {
          ctx.has_error = true;
          co_return;
        }

        co_yield create_vm_map(desc, lvl, vaddr);
        break;
    }

    // Move right
    vaddr.val = bits::Clear(vaddr.val, 0, OUTPUT_ADDRESS_OFFSET_ON_LEVEL[lvl]);
    vaddr.val += VA_SPACING_PER_ENTRY[lvl];
  }

  free_empty_descs(root, ctx);
}

#if USE_THREAD_LOCAL == 1
static thread_local DefaultStackAllocator coro_alloc;
#else
static DefaultStackAllocator coro_alloc;
#endif

void BeginTraversal(TraverseContext &ctx) {
  coro_alloc.used = 0;

  StackAllocator<yield_t::promise_type> alloc{
      bit_cast<DefaultStackAllocator *>(&coro_alloc)};

  auto &root = *bit_cast<const DescriptorTable *>(ctx.root_desc);

  // Move the allocated generator into the rust provided stack space.
  auto gen = Traverse(std::allocator_arg, alloc, ctx, root, ctx.va_start, 0);
  std::construct_at(bit_cast<yield_t *>(&ctx.gen), std::move(gen));
}

VMMap NextItem(TraverseContext &ctx) {
  auto &gen = *bit_cast<yield_t *>(&ctx.gen);
  if (gen) [[likely]] {
    return gen();
  } else {
    ctx.done = true;
    return {};
  }
}

void EndTraversal(TraverseContext &ctx) {
  std::destroy_at(bit_cast<yield_t *>(&ctx.gen));
}
}  // namespace mmu::tt

[[noreturn]] void terminate() {
  mmu::tt::terminate();
  __builtin_unreachable();
}
}  // namespace mei

void operator delete(void *) noexcept { mei::terminate(); }
void operator delete(void *, std::size_t /* n */) noexcept { mei::terminate(); }
void *operator new(size_t /* sz */) { mei::terminate(); }