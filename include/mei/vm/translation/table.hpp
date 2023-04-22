#pragma once

#include "control.hpp"
#include "generated/mmu.hpp"

namespace mei::vm::translation {
namespace detail {
using namespace registers::mmu;
}

template<control_like Control>
struct alignas(granule_size<Control>) DescriptorTable {
  using control = Control;
  using desc_t = detail::STAGE1_TABLE_DESCRIPTOR_t::word_type;

  std::array<desc_t, max_bits_per_level<Control>> data;
};

namespace test {
using desc_table = DescriptorTable<test::cntrl>;

static_assert(sizeof(desc_table) == granule_size<desc_table::control>);
static_assert(alignof(desc_table) == granule_size<desc_table::control>);
}  // namespace test
}  // namespace mei::vm::translation