#pragma once

#include "access.h"
#include "field.h"
#include "mei/fmt/format.h"
#include "register.h"

namespace mei::dtl {
template <mei::registers::register_t R,
          mei::registers::field_of<R> F,
          mei::registers::field_value_of<R> FV>
class real_field_value {
 public:
  using Field = F;

  explicit constexpr real_field_value(const FV& v) : m_fv(&v) {}

  constexpr auto Get() const -> const FV& { return *m_fv; }

 private:
  const FV* m_fv;
};

template <typename>
struct format_fields;

template <mei::registers::field F, typename... Fs>
  requires((std::same_as<REG(Fs), REG(F)> && ...))
struct format_fields<std::tuple<F, Fs...>> {
  using R = REG(F);

  static constexpr auto format(const mei::registers::register_storage_for<R> auto& rs,
                               fmt::format_context& ctx) {
    using FV = typename F::Value;
    using RFV = real_field_value<R, F, FV>;

    FV fv{mei::registers::Read<F>(rs)};

    fmt::formatter<real_field_value<R, F, FV>>().format(RFV{fv}, ctx);

    if constexpr (sizeof...(Fs) != 0) {
      fmt::format_to(ctx.out(), ", ");
      return format_fields<std::tuple<Fs...>>::format(rs, ctx);
    } else {
      return ctx.out();
    }
  }
};
}  // namespace mei::dtl

template <mei::registers::register_t R,
          mei::registers::field_of<R> F,
          mei::registers::field_value_of<R> FV>
struct fmt::formatter<mei::dtl::real_field_value<R, F, FV>> : formatter<INTT(R)> {
  using base = formatter<INTT(R)>;
  using type = mei::dtl::real_field_value<R, F, FV>;

  template <typename ParseContext>
  constexpr auto parse(ParseContext& ctx) {
    return ctx;
  }

  constexpr auto format(const type& rfv, fmt::format_context& ctx) {
    const auto& fv = rfv.Get();
    format_to(ctx.out(), "{{{}: ", F::Name);
    if constexpr (mei::registers::enum_field<F>) {
      format_to(ctx.out(), "{}", F::EnumValueToStr(fv.Val()));
    } else {
      base::format(fv.Val(), ctx);
    }
    format_to(ctx.out(), "}}");
    return ctx.out();
  }
};

template <mei::registers::register_storage RS>
  requires(mei::registers::formattable_register<REG(RS)>)
struct fmt::formatter<RS> : formatter<REG_INTT(RS)> {
  constexpr auto format(const RS& rs, fmt::format_context& ctx) {
    using R = REG(RS);

    format_to(ctx.out(), "{}: [", R::Name);
    mei::dtl::format_fields<typename R::FieldTypes>::format(rs, ctx);
    format_to(ctx.out(), "]", R::Name);
    return ctx.out();
  }
};