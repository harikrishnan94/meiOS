#pragma once

#include "control.hpp"
#include "generated/mmu.hpp"

namespace mei::vm::translation {
namespace regs {
using namespace registers::mmu;
}

using desc_t = regs::STAGE1_TABLE_DESCRIPTOR_t::word_type;

namespace detail {
using last_level_descriptor = registers::LocalCopyRegister<regs::STAGE1_LAST_LEVEL_DESCRIPTOR>;
using table_descriptor = registers::LocalCopyRegister<regs::STAGE1_TABLE_DESCRIPTOR>;
using block_descriptor = registers::LocalCopyRegister<regs::STAGE1_BLOCK_DESCRIPTOR>;
using page_descriptor = registers::LocalCopyRegister<regs::STAGE1_PAGE_DESCRIPTOR>;
}  // namespace detail

template<typename Ops>
concept desc_ops_like = requires(
    detail::table_descriptor& tdesc,
    detail::page_descriptor& pdesc,
    detail::block_descriptor& bdesc,
    ktl::u32 level) {
  { Ops::template get_next_level_desc<0>(tdesc) } -> std::convertible_to<uintptr_t>;
  { Ops::template set_next_level_desc<0>(tdesc, uintptr_t {}) };
  { Ops::template get_output_address<0>(pdesc) } -> std::convertible_to<uintptr_t>;
  { Ops::template set_output_address<0>(pdesc, uintptr_t {}) };
  { Ops::template get_output_address<0>(bdesc) } -> std::convertible_to<uintptr_t>;
  { Ops::template set_output_address<0>(bdesc, uintptr_t {}) };
};

template<typename Ops, typename Control>
concept desc_ops_for = control_like<Control> && desc_ops_like<Ops> && requires {
  { std::same_as<typename Ops::control, Control> };
};
}  // namespace mei::vm::translation