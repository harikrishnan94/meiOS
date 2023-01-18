#include "translation_table.h"

#include <algorithm>

#include "bits.h"
#include "generator.h"

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

  NEXT_LEVEL_DESC_OFFSET = 12,
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
        co_yield Traverse(std::allocator_arg, alloc, ctx,
                          *bit_cast<const DescriptorTable *>(child), vaddr,
                          lvl + 1);
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

static DefaultStackAllocator coro_alloc;

void BeginTraversal(TraverseContext &ctx) {
  coro_alloc.used = 0;

  StackAllocator<yield_t::promise_type> alloc{
      bit_cast<DefaultStackAllocator *>(&coro_alloc)};
  StackAllocator<yield_t> gen_alloc{
      bit_cast<DefaultStackAllocator *>(&coro_alloc)};

  yield_t *gen_ptr = gen_alloc.allocate(1);

  auto &root = *bit_cast<const DescriptorTable *>(ctx.root_desc);

  // Move the allocated generator into the rust provided stack space.
  auto gen = Traverse(std::allocator_arg, alloc, ctx, root, ctx.va_start, 0);
  std::construct_at(gen_ptr, yield_t{std::move(gen)});

  ctx.gen_ptr = bit_cast<size_t>(gen_ptr);
}

VMMap NextItem(TraverseContext &ctx) {
  auto gen = bit_cast<yield_t *>(ctx.gen_ptr);
  if (gen->move_next()) {
    return gen->current_value();
  } else {
    ctx.done = true;
    return {};
  }
}

void EndTraversal(TraverseContext &ctx) {
  std::destroy_at(bit_cast<yield_t *>(ctx.gen_ptr));
}
}  // namespace mei::mmu::tt

void operator delete(void *) noexcept { mei::terminate(); }
void operator delete(void *, std::size_t /* n */) noexcept { mei::terminate(); }
void *operator new(size_t /* sz */) { mei::terminate(); }