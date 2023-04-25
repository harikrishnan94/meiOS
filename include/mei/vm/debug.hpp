#pragma once

#include <ktl/fmt/core.hpp>

#include <mei/vm/memory_map_desc.hpp>

namespace ktl::fmt {
template<typename CharT>
struct formatter<CharT, mei::vm::AccessPermissions> {
  template<typename FormatContext, typename FmtSpec>
    requires std::same_as<CharT, typename FormatContext::char_type>
  constexpr auto format(
      FormatContext& ctx,
      const FmtSpec& /* fmt_spec */,
      const mei::vm::AccessPermissions& access_perms) noexcept -> expected<bool, Error> {
    return ctx.template Format<
        "{{el0_read = {}, el0_write = {}, el0_execute = {}, el1_read = {}, el1_write = {}, el1_execute = {}}}">(
        access_perms.el0_read,
        access_perms.el0_write,
        access_perms.el0_execute,
        access_perms.el1_read,
        access_perms.el1_write,
        access_perms.el1_execute);
  }
};

template<typename CharT>
struct formatter<CharT, mei::vm::MapDesc> {
  template<typename FormatContext, typename FmtSpec>
    requires std::same_as<CharT, typename FormatContext::char_type>
  constexpr auto format(
      FormatContext& ctx,
      const FmtSpec& /* fmt_spec */,
      const mei::vm::MapDesc& map_desc) noexcept -> expected<bool, Error> {
    return ctx
        .template Format<"[phy_addr: {}, virt_addr: {}, num_pages: {}, access_permissions = {}]">(
            map_desc.phy_addr.value,
            map_desc.virt_addr.value,
            map_desc.num_pages,
            map_desc.access_perms);
  }
};

template<typename CharT>
struct formatter<CharT, mei::vm::MemoryMap> {
  template<typename FormatContext, typename FmtSpec>
    requires std::same_as<CharT, typename FormatContext::char_type>
  constexpr auto
  format(FormatContext& ctx, const FmtSpec& /* fmt_spec */, const mei::vm::MemoryMap& map) noexcept
      -> expected<bool, Error> {
    if (map.kind == mei::vm::MemoryKind::Normal) {
      return ctx.template Format<"Normal Memory Map: {}">(map.desc);
    }
    return ctx.template Format<"Device Memory Map: {}">(map.desc);
  }
};
}  // namespace ktl::fmt