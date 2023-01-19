#include <algorithm>
#include <cinttypes>

#include "bits.h"
#include "generator.h"
#include "libmei/src/mmu/translation_table.rs.h"

#ifndef NDEBUG
#define assert_x(cond) \
  if (!(cond)) terminate();
#else
#define assert_x(cond)
#endif

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

static constexpr u64 FOUR_KIB = 4 * 1024;

enum TranslationConsts : u64 {
  VIRTUAL_ADDRESS_IGNORE_MSB = 16,
  VIRTUAL_ADDRESS_LEVEL_IDX_BITS = 9,
  VIRTUAL_ADDRESS_NBITS =
      std::countr_zero(VirtualAddress{0}.val) - VIRTUAL_ADDRESS_IGNORE_MSB,
  VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS = std::countr_one(FOUR_KIB - 1),
  MAX_TRANSLATION_LEVELS =
      (VIRTUAL_ADDRESS_NBITS - VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS) /
      VIRTUAL_ADDRESS_LEVEL_IDX_BITS,
  NUM_TABLE_DESC_ENTRIES = (1ULL << VIRTUAL_ADDRESS_LEVEL_IDX_BITS)
};

enum DescriptorConsts : u32 {
  DESCRIPTOR_VALID_OFFSET = 0,
  DESCRIPTOR_TYPE_OFFSET = 1,

  DESCRIPTOR_ENTRY_SIZE = sizeof(reg),
  NEXT_LEVEL_DESC_OFFSET = std::countr_zero(
      (1 << VIRTUAL_ADDRESS_LEVEL_IDX_BITS) * DESCRIPTOR_ENTRY_SIZE),
  NEXT_LEVEL_DESC_NBITS = VIRTUAL_ADDRESS_NBITS - NEXT_LEVEL_DESC_OFFSET,
};

enum class DescriptorType { INVALID, TABLE, BLOCK, PAGE };

static constexpr auto va_spacing_per_entry(Level lvl) {
  return FOUR_KIB << (MAX_TRANSLATION_LEVELS - 1 - lvl) *
                         VIRTUAL_ADDRESS_LEVEL_IDX_BITS;
}

static constexpr auto output_address_offest(Level lvl) {
  return VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS +
         (MAX_TRANSLATION_LEVELS - 1 - lvl) * VIRTUAL_ADDRESS_LEVEL_IDX_BITS;
}

static constexpr auto output_address_nbits(Level lvl) {
  return VIRTUAL_ADDRESS_NBITS - output_address_offest(lvl);
}

struct DescriptorTable {
  std::array<reg, NUM_TABLE_DESC_ENTRIES> descs;
};

static constexpr size_t get_index_for_level(VirtualAddress vaddr, Level lvl) {
  auto offset =
      VIRTUAL_ADDRESS_PAGE_OFFSET_NBITS +
      (MAX_TRANSLATION_LEVELS - 1 - lvl) * VIRTUAL_ADDRESS_LEVEL_IDX_BITS;
  return bits::Get(vaddr.val, offset, VIRTUAL_ADDRESS_LEVEL_IDX_BITS) >> offset;
}

static constexpr auto parse_descriptor(reg desc, Level level)
    -> DescriptorType {
  if (!bits::Get(desc, DESCRIPTOR_VALID_OFFSET, 1))
    return DescriptorType::INVALID;

  if (bits::Get(desc, DESCRIPTOR_TYPE_OFFSET, 1)) {
    // Table or Page
    return level == MAX_TRANSLATION_LEVELS - 1 ? DescriptorType::PAGE
                                               : DescriptorType::TABLE;
  } else {
    assert_x(level == 1 || level == 2);
    // Block
    return DescriptorType::BLOCK;
  }
}

static auto create_vm_map(const reg &desc, Level lvl, VirtualAddress vaddr)
    -> VMMap {
  auto offset = output_address_offest(lvl);
  auto output_address = bits::Get(desc, offset, output_address_nbits(lvl));
  auto block_len = va_spacing_per_entry(lvl);

  vaddr.val = bits::Clear(vaddr.val, 0, offset);

  return {{output_address}, block_len, vaddr, bit_cast<reg>(&desc)};
}

static constexpr void free_table_if_empty(const DescriptorTable &table,
                                          TraverseContext &ctx) {
  if (ctx.free_empty_descs) {
    bool is_empty =
        std::all_of(table.descs.begin(), table.descs.end(), [](auto desc) {
          return bits::Get(desc, DESCRIPTOR_VALID_OFFSET, 1) == 0;
        });

    if (is_empty) {
      ctx.empty_descs[ctx.num_empty_descs] = bit_cast<u64>(&table);
      ctx.num_empty_descs++;
    }
  }
}

static constexpr auto move_vaddr_right(VirtualAddress vaddr, Level lvl)
    -> VirtualAddress {
  vaddr.val = bits::Clear(vaddr.val, 0, output_address_offest(lvl));
  vaddr.val += va_spacing_per_entry(lvl);
  return vaddr;
}

static constexpr auto LEVEL_NBITS = std::countr_one(MAX_TRANSLATION_LEVELS - 1);

static auto form_desc_table_ptr(const DescriptorTable *table, Level lvl,
                                u32 idx) -> reg {
  auto desc_ptr = bit_cast<reg>(table);
  bits::Clear(desc_ptr, 0, NEXT_LEVEL_DESC_OFFSET);
  desc_ptr |=
      bits::Get(lvl, 0, LEVEL_NBITS) |
      (bits::Get(idx, 0, VIRTUAL_ADDRESS_LEVEL_IDX_BITS) << LEVEL_NBITS);

  return desc_ptr;
}

static auto deform_desc_table_ptr(reg descs_ptr)
    -> std::tuple<const DescriptorTable *, Level, u32> {
  auto table = bit_cast<const DescriptorTable *>(
      bits::Get(descs_ptr, NEXT_LEVEL_DESC_OFFSET, NEXT_LEVEL_DESC_NBITS));
  auto lvl = bits::Get(descs_ptr, 0, LEVEL_NBITS);
  auto idx =
      bits::Get(descs_ptr, LEVEL_NBITS, VIRTUAL_ADDRESS_LEVEL_IDX_BITS) >>
      LEVEL_NBITS;

  return {table, lvl, idx};
}

static auto skip_invalid_entries(TraverseContext &ctx, VirtualAddress &vaddr,
                                 const DescriptorTable *table, Level lvl,
                                 u32 idx) -> u32 {
  while (idx < NUM_TABLE_DESC_ENTRIES && vaddr.val < ctx.va_end.val) {
    auto desc = table->descs[idx];
    auto type = parse_descriptor(desc, lvl);

    if (type != DescriptorType::INVALID) break;

    vaddr = move_vaddr_right(vaddr, lvl);
    idx++;
  }

  return idx;
}

// create 0th level and return it.
static auto begin(TraverseContext &ctx, VirtualAddress &vaddr) -> reg {
  const auto *table = bit_cast<const DescriptorTable *>(ctx.root_desc);
  auto idx = get_index_for_level(vaddr, 0);

  return form_desc_table_ptr(table, 0, idx);
}

static auto get(VirtualAddress vaddr, reg descs_ptr, VMMap &map) -> bool {
  auto [table, lvl, idx] = deform_desc_table_ptr(descs_ptr);
  auto &desc = table->descs[idx];
  auto type = parse_descriptor(desc, lvl);

  if (type == DescriptorType::TABLE || type == DescriptorType::INVALID)
    return false;

  map = create_vm_map(desc, lvl, vaddr);

  return true;
}

using descs_stash_t = std::array<reg, MAX_TRANSLATION_LEVELS - 1>;

static auto next(TraverseContext &ctx, reg descs_ptr, VirtualAddress &vaddr,
                 descs_stash_t &stash) -> reg {
  auto [table, lvl, idx] = deform_desc_table_ptr(descs_ptr);
  idx = skip_invalid_entries(ctx, vaddr, table, lvl, idx);

  if (vaddr.val >= ctx.va_end.val) return 0;

  auto child_desc = table->descs[idx];
  auto child_type = parse_descriptor(child_desc, lvl);

  if (child_type == DescriptorType::TABLE) {
    // stash us.
    stash[lvl] = descs_ptr;

    // Move down.
    lvl += 1;
    idx = get_index_for_level(vaddr, lvl);

    auto child =
        bits::Get(child_desc, NEXT_LEVEL_DESC_OFFSET, NEXT_LEVEL_DESC_NBITS);

    return form_desc_table_ptr(bit_cast<const DescriptorTable *>(child), lvl,
                               idx);
  } else {
    // Move both the desc_ptr and vaddr right.
    idx += 1;
    vaddr = move_vaddr_right(vaddr, lvl);
    descs_ptr = form_desc_table_ptr(table, lvl, idx);

    if (vaddr.val >= ctx.va_end.val) return 0;

    if (idx == NUM_TABLE_DESC_ENTRIES) {
      // Move up, until we find a parent.
      while (lvl != 0) {
        free_table_if_empty(*table, ctx);

        auto [parent_table, parent_lvl, parent_idx] =
            deform_desc_table_ptr(stash[lvl - 1]);

        if (parent_idx + 1 < NUM_TABLE_DESC_ENTRIES)
          return form_desc_table_ptr(parent_table, parent_lvl, parent_idx + 1);

        // Fallen off the parent too.. Move up again.
        lvl = parent_lvl;
      }

      return 0;
    }

    return descs_ptr;
  }
}

using yield_t = generator<VMMap>;

static auto Traverse(std::allocator_arg_t,
                     StackAllocator<yield_t::promise_type> /* alloc */,
                     TraverseContext &ctx, VirtualAddress vaddr) -> yield_t {
  auto descs_ptr = begin(ctx, vaddr);
  descs_stash_t stash;

  while (descs_ptr) {
    VMMap map;

    if (get(vaddr, descs_ptr, map)) co_yield map;

    descs_ptr = next(ctx, descs_ptr, vaddr, stash);
  }
}

void BeginTraversal(TraverseContext &ctx) {
  static_assert(sizeof(DefaultStackAllocator) == sizeof(ctx.traverse_stack));

  auto *coro_alloc =
      reinterpret_cast<DefaultStackAllocator *>(ctx.traverse_stack.data());
  coro_alloc->used = 0;

  StackAllocator<yield_t::promise_type> alloc{
      bit_cast<DefaultStackAllocator *>(coro_alloc)};

  // Move the allocated generator into the rust provided stack space.
  auto gen = Traverse(std::allocator_arg, alloc, ctx, ctx.va_start);
  std::construct_at(bit_cast<yield_t *>(&ctx.gen), std::move(gen));
}

auto NextItem(TraverseContext &ctx) -> VMMap {
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