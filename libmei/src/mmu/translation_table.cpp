#include <experimental/coroutine>

#include "bits.h"
#include "generator.h"
#include "libmei/src/mmu/translation_table.h"

namespace mei::mmu::tt {
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

enum DescriptorBits : u32 {
  DESCRIPTOR_VALID_OFFSET = 1,
  DESCRIPTOR_TYPE_OFFSET = 1,

  OUTPUT_ADDR_4KIB_OFFSET = 12,
  OUTPUT_ADDR_4KIB_NBITS = 36,

  OUTPUT_ADDR_2MIB_OFFSET = 21,
  OUTPUT_ADDR_2MIB_NBITS = 27,

  OUTPUT_ADDR_1GIB_OFFSET = 30,
  OUTPUT_ADDR_1GIB_NBITS = 18,

  NEXT_LEVEL_DESC_OFFSET = 12,
  NEXT_LEVEL_DESC_NBITS = 36,
};

enum Translation : u64 {
  NUM_LEVELS = 4,
  VIRTUAL_ADDRESS_NBITS = std::countr_zero(VirtualAddress{0}.val),
  VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS = std::countr_one(FOUR_KIB - 1),
  VIRTUAL_ADDRESS_IGNORE_MSB = 16,
  VIRTUAL_ADDRESS_LEVEL_IDX_BITS = 9,
  NUM_TABLE_DESC_ENTRIES = (1ULL << VIRTUAL_ADDRESS_LEVEL_IDX_BITS)
};

enum class DescriptorType { INVALID = 0, TABLE = 1, BLOCK = 2, PAGE = 3 };

static constexpr u64 VA_SPACING_PER_ENTRY[] = {
    512ULL * 1024 * 1024 * 1024, 1024 * 1024 * 1024, 2 * 1024 * 1024, 4 * 1024};

static constexpr u64 OUTPUT_ADDRESS_OFFSET_ON_LEVEL[] = {
    0, OUTPUT_ADDR_1GIB_OFFSET, OUTPUT_ADDR_2MIB_OFFSET,
    OUTPUT_ADDR_4KIB_OFFSET};
static constexpr u64 OUTPUT_ADDRESS_NBITS_ON_LEVEL[] = {
    0, OUTPUT_ADDR_1GIB_NBITS, OUTPUT_ADDR_2MIB_NBITS, OUTPUT_ADDR_4KIB_NBITS};

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

static constexpr auto create_vm_map(const reg &desc, Level lvl,
                                    VirtualAddress &vaddr_start) -> VMMap {
  auto offset = OUTPUT_ADDRESS_OFFSET_ON_LEVEL[lvl];
  auto output_address =
      bits::Get(desc, offset, OUTPUT_ADDRESS_NBITS_ON_LEVEL[lvl]);
  auto block_len = VA_SPACING_PER_ENTRY[lvl];

  vaddr_start.val = bits::Clear(vaddr_start.val, 0, offset);

  return {PhysicalAddress{output_address}, block_len, vaddr_start,
          bit_cast<reg>(&desc)};
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

static auto Traverse(TraverseContext &ctx, const DescriptorTable &root,
                     VirtualAddress vaddr_start, VirtualAddress vaddr_end,
                     Level lvl) -> generator<VMMap> {
  if (lvl == NUM_LEVELS) {
    ctx.has_error = true;
    co_return;
  }

  for (auto ind = get_index_for_level(vaddr_start, lvl);
       ind < NUM_TABLE_DESC_ENTRIES && vaddr_start.val >= vaddr_end.val;
       ind++) {
    const auto &desc = root.descs[ind];
    auto type = parse_descriptor(desc, lvl);

    switch (type) {
    case DescriptorType::INVALID:
      break;

    case DescriptorType::TABLE: {
      auto child =
          bits::Get(desc, NEXT_LEVEL_DESC_OFFSET, NEXT_LEVEL_DESC_NBITS) >>
          NEXT_LEVEL_DESC_OFFSET;

      // Descend down.
      co_yield Traverse(ctx, *bit_cast<const DescriptorTable *>(child),
                        vaddr_start, vaddr_end, lvl + 1);
      break;
    }

    case DescriptorType::BLOCK:
      if (lvl == 0 || lvl == 3) {
        ctx.has_error = true;
        co_return;
      }

      co_yield create_vm_map(desc, lvl, vaddr_start);
      break;

    case DescriptorType::PAGE:
      if (lvl != 3) {
        ctx.has_error = true;
        co_return;
      }

      co_yield create_vm_map(desc, lvl, vaddr_start);
      break;
    }

    // Move right
    vaddr_start.val += VA_SPACING_PER_ENTRY[lvl];
  }

  free_empty_descs(root, ctx);
}

static inline auto GEN(TraverseContext &ctx) {
  return bit_cast<generator<VMMap> *>(ctx.traverse_stack.data());
}

void BeginTraversal(TraverseContext &ctx) {
  auto &root = *bit_cast<const DescriptorTable *>(ctx.root_desc);
  new (GEN(ctx))
      generator<VMMap>{Traverse(ctx, root, ctx.va_start, ctx.va_end, 0)};
}

VMMap NextItem(TraverseContext &ctx) {
  auto gen = GEN(ctx);
  if (gen->move_next()) {
    return gen->current_value();
  } else {
    ctx.done = true;
    return {};
  }
}

void EndTraversal(TraverseContext &ctx) { std::destroy_at(GEN(ctx)); }
} // namespace mei::mmu::tt
