#pragma once

#include <mei/result.hpp>
#include <mei/vm/memory_map_desc.hpp>

#include "control.hpp"
#include "desc_ops.hpp"
#include "generated/mmu.hpp"

namespace mei::vm::translation {
template<control_like Control>
struct alignas(granule_size<Control>) DescriptorTable {
  using control = Control;

  static const desc_t INVALID_DESCRIPTOR = 0;
  static_assert(!registers::LocalCopyRegister<regs::STAGE1_PAGE_DESCRIPTOR> {INVALID_DESCRIPTOR}
                     .IsSet(regs::STAGE1_PAGE_DESCRIPTOR.Valid));

  std::array<desc_t, max_bits_per_level<Control>> data = {};
};

namespace error {
DEFINE_ERROR(InvalidVirtualAddress);
DEFINE_ERROR_EXT(CorruptedTable, "Corrupted Translation Table");
}

namespace detail {
struct invalid_descriptor {
  desc_t val;
};

// Any descriptor
using descriptor =
    std::variant<invalid_descriptor, table_descriptor, block_descriptor, page_descriptor>;

// Is a descriptor valid?
static constexpr auto is_valid(desc_t desc) noexcept {
  // Any descriptor type can be used here, to check if a descriptor is valid.
  return table_descriptor {desc}.IsSet(regs::STAGE1_TABLE_DESCRIPTOR.Valid);
}

static constexpr auto
to_raw_attributes(AccessPermissions access_perms, MemoryKind mem_kind) noexcept -> desc_t {
  last_level_descriptor desc {};
  bool el1_ro = access_perms.el1_read;
  auto el1_rw = access_perms.el1_read && access_perms.el1_write;
  bool el0_ro = access_perms.el0_read;
  auto el0_rw = access_perms.el0_read && access_perms.el0_write;

  if (el1_rw) {
    if (el0_rw) {
      desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.AP.RW_EL1_EL0);
    } else {
      desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.AP.RW_EL1);
    }
  } else if (el1_ro) {
    if (el0_ro) {
      desc.Modify(regs::STAGE1_PAGE_DESCRIPTOR.AP.RO_EL1_EL0);
    } else {
      desc.Modify(regs::STAGE1_PAGE_DESCRIPTOR.AP.RO_EL1);
    }
  }

  if (access_perms.el1_write || !access_perms.el1_execute) {
    desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.PXN.SET);
  }
  if (access_perms.el0_write || !access_perms.el0_execute) {
    desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.UXN.SET);
  }

  if (mem_kind == MemoryKind::Normal) {
    desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.SH.InnerShareable);
  } else {
    desc.Modify(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.SH.OuterShareable);
  }

  return desc.Get();
}

static constexpr auto decode_last_level_desc(desc_t desc) noexcept
    -> std::pair<AccessPermissions, MemoryKind> {
  AccessPermissions access_perms = {};
  last_level_descriptor ap {desc};

  switch (ktl::unwrap(ap.ReadEnum(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.AP))) {
    using enum regs::STAGE1_LAST_LEVEL_DESCRIPTOR_t::AP_def::Enum;
    case RW_EL1_EL0:
      access_perms.el0_read = true;
      access_perms.el0_write = true;
      [[fallthrough]];
    case RW_EL1:
      access_perms.el1_read = true;
      access_perms.el1_write = true;
      break;

    case RO_EL1_EL0:
      access_perms.el0_read = true;
      [[fallthrough]];
    case RO_EL1:
      access_perms.el1_read = true;
      break;
  }

  if (!ap.MatchesAny(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.PXN.True) && !access_perms.el1_write) {
    access_perms.el1_execute = true;
  }
  if (!ap.MatchesAny(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.UXN.True) && !access_perms.el0_write) {
    access_perms.el0_execute = true;
  }

  bool is_cacheable = !ap.MatchesAny(regs::STAGE1_LAST_LEVEL_DESCRIPTOR.SH.OuterShareable);
  return {access_perms, is_cacheable ? MemoryKind::Normal : MemoryKind::Device};
}

// Decode a descriptor (as either Table, Block or Page) and process it accordingly.
// Level information is needed to distinguish b/w table and page descriptor.
// If a Block descriptor is found in-appropriately level, throw error.
template<desc_ops_like Ops, ktl::u32 Level, typename Result>
  requires ktl::detail::is_expected<Result>::value
static constexpr auto process_desc(
    desc_t desc,
    auto&& table_descriptor_cb,
    auto&& block_descriptor_cb,
    auto&& page_descriptor_cb,
    auto&& invalid_descriptor_cb) noexcept -> Result {
  table_descriptor tbl_desc {desc};
  // Is Valid?
  if (!tbl_desc.IsSet(regs::STAGE1_TABLE_DESCRIPTOR.Valid)) {
    return invalid_descriptor_cb(invalid_descriptor {desc});
  }

  // Table and Page descriptors both have bits [0:1] set to 0b11.
  // Only way they can be distinguished is at which level they appear.
  if (tbl_desc.MatchesAny(regs::STAGE1_TABLE_DESCRIPTOR.TYPE.Table)) {
    // Last level descriptors must be a page descriptor
    if constexpr (Level == num_levels<typename Ops::control> - 1) {
      return page_descriptor_cb(page_descriptor {desc});
    } else {
      return table_descriptor_cb(tbl_desc);
    }
  }

  // Must be a block descriptor
  if constexpr (Ops::can_have_block_desc_at(Level)) {
    return block_descriptor_cb(block_descriptor {desc});
  } else {
    Throw(error::CorruptedTable);
  }
}

template<
    desc_ops_like Ops,
    ktl::u32 Level,
    bool IsConst,
    typename DescTable = DescriptorTable<typename Ops::control>,
    typename Ptr =
        std::add_pointer_t<std::conditional_t<IsConst, std::add_const_t<DescTable>, DescTable>>>
static auto descend_tree(table_descriptor tdesc) -> ktl::not_null<Ptr> {
  return std::bit_cast<Ptr>(Ops::template get_next_level_desc<Level>(tdesc));
}

// Virt2Phy recursive helper implementation.
template<
    desc_ops_like Ops,
    ktl::u32 Level,
    typename DescriptorTable = DescriptorTable<typename Ops::control>,
    typename result = Result<MemoryMap>>
[[nodiscard]] static auto lookup(const DescriptorTable& root, VirtualAddress vaddr) -> result {
  auto to_memory_map = [&](auto desc, uintptr_t paddr) {
    auto [access_perms, mem_kind] = decode_last_level_desc(desc.Get());
    MapDesc map_desc =
        {.phy_addr = {paddr}, .virt_addr = vaddr, .num_pages = 1, .access_perms = access_perms};
    return MemoryMap {.kind = mem_kind, .desc = map_desc};
  };

  auto idx = GetIndexForLevel<typename Ops::control, Level>(vaddr);
  auto desc = ktl::at(root.data, idx);

  return process_desc<Ops, Level, result>(
      desc,
      [&](auto tdesc) {
        const auto& children = *descend_tree<Ops, Level, true>(tdesc);
        return lookup<Ops, Level + 1>(children, vaddr);
      },
      [&](auto bdesc) {
        return to_memory_map(bdesc, Ops::template get_output_address<Level>(bdesc));
      },
      [&](auto pdesc) {
        return to_memory_map(pdesc, Ops::template get_output_address<Level>(pdesc));
      },
      [](auto /* desc */) -> result { return {}; });
}
}  // namespace detail

// Walk the translation table using the VirtualAddress `vaddr` and produce corresponding
// PhysicalAddress This is similar to what CPU does after a TLB Miss.
template<desc_ops_like Ops, control_like Control = typename Ops::control>
inline auto Virt2Phy(const DescriptorTable<Control>& root, VirtualAddress vaddr) noexcept
    -> Result<MemoryMap> {
  if (!IsValid<Control>(vaddr))
    Throw(error::InvalidVirtualAddress);
  return detail::lookup<Ops, 0>(root, vaddr);
}

namespace test {
using desc_table = DescriptorTable<test::cntrl>;

static_assert(sizeof(desc_table) == granule_size<desc_table::control>);
static_assert(alignof(desc_table) == granule_size<desc_table::control>);
}  // namespace test
}  // namespace mei::vm::translation