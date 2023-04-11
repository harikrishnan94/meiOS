#pragma once

#include <concepts>
#include <limits>
#include <tuple>
#include <type_traits>

#include <ktl/bitops.hpp>
#include <ktl/fmt/format.hpp>
#include <ktl/int.hpp>
#include <ktl/memory.hpp>
#include <ktl/string_view.hpp>
#include <ktl/utility.hpp>

namespace mei::registers {
namespace detail {
template<typename T>
struct is_tuple {
  static constexpr bool value = false;
};

template<typename... Ts>
struct is_tuple<std::tuple<Ts...>> {
  static constexpr bool value = true;
};

template<typename R>
concept register_like = std::is_empty_v<R> && std::is_trivial_v<R> && requires {
  { std::integral<typename R::word_type> };
  { std::same_as<decltype(R::name), ktl::string_view> };
  { is_tuple<typename R::field_types>::value };
};

template<ktl::const_string Str>
struct named_string {
  constexpr auto begin() const noexcept {
    return Str.begin();
  }

  constexpr auto end() const noexcept {
    return Str.end();
  }

  constexpr auto view() const noexcept {
    return Str.view();
  }
};
}  // namespace detail

template<typename F>
concept field_like = std::is_empty_v<F> && std::is_default_constructible_v<F> && requires {
  { detail::register_like<typename F::register_type> };
  { std::same_as<decltype(F::name), ktl::string_view> };
  { std::integral<std::decay_t<decltype(F::offset::value)>> };
  { std::integral<std::decay_t<decltype(F::numbits::value)>> };
  { F::numbits::value != 0 };
  {
    F::offset::value + F::numbits::value
        <= std::numeric_limits<typename F::register_type::word_type>::digits
  };
  { std::declval<F>()(0) };
};

template<typename F, typename Register>
concept field_of = field_like<F> && std::same_as<typename F::register_type, Register>;

namespace detail {
template<typename R, typename T>
struct fields_belong_to_me;

template<typename R, typename... Fs>
struct fields_belong_to_me<R, std::tuple<Fs...>> {
  static constexpr bool value = (field_of<Fs, R> && ...);
};
}  // namespace detail

template<typename R>
concept register_like =
    detail::register_like<R> && detail::fields_belong_to_me<R, typename R::field_types>::value;

/// Field Traits
template<field_like F>
struct field_traits {
 public:
  using field_type = F;
  using register_type = F::register_type;
  using word_type = register_type::word_type;

 private:
  template<typename Field>
  struct find_enum_type {
    struct field {
      using Enum = void;
    };
  };

  template<typename Field>
    requires(std::is_enum_v<typename F::Enum>)
  struct find_enum_type<Field> {
    using field = Field;
  };

 public:
  using enum_type = find_enum_type<F>::field::Enum;

  static constexpr auto is_enum =
      std::bool_constant<std::is_enum_v<enum_type>&& requires(word_type enum_val) {
        { F::EnumStr(enum_val) } -> std::same_as<std::optional<ktl::string_view>>;
        { F::IsValid(enum_val) } -> std::same_as<bool>;
      }>::value;

  static constexpr ktl::u8 offset = field_type::offset::value;
  static constexpr ktl::u8 numbits = field_type::numbits::value;
  static constexpr auto mask = ktl::CreateMask<word_type>(offset, numbits);
  static constexpr auto name = field_type::name;

  static constexpr auto IsValid(word_type val) noexcept -> bool {
    if constexpr (is_enum) {
      return F::IsValid(val >> offset);
    } else {
      return val == GetMaskedBits(val, mask);
    }
  }

  static constexpr auto NaturalValue(word_type val) noexcept -> word_type {
    return val >> offset;
  }

  static constexpr auto Set(word_type regval, word_type field_val) noexcept -> word_type {
    return SetMaskedBits(regval, field_val, mask);
  }

  static constexpr auto Contains(word_type regval, word_type field_val) noexcept -> bool {
    return ktl::EqualsInMask(regval, field_val, mask);
  }

  static constexpr auto AsEnum(word_type val) noexcept -> std::optional<enum_type>
    requires(is_enum)
  {
    if (IsValid(val))
      return AsEnumUnchecked(val);
    return {};
  }
  static constexpr auto AsEnumUnchecked(word_type val) noexcept -> enum_type
    requires(is_enum)
  {
    check_(IsValid(val), "invalid enum value for field");
    return static_cast<enum_type>(val);
  }

  static constexpr auto EnumStr(word_type val) noexcept -> std::optional<ktl::string_view>
    requires(is_enum)
  {
    return F::EnumStr(val);
  }
};

template<std::integral WordType, ktl::const_string Name>
struct GenericRegister {
  using word_type = WordType;

  static constexpr detail::named_string<Name> name = {};
};

template<typename Register, ktl::usize Offset, ktl::usize NumBits, ktl::const_string Name>
struct GenericField {
  using register_type = Register;
  using offset = std::integral_constant<ktl::usize, Offset>;
  using numbits = std::integral_constant<ktl::usize, NumBits>;

  static constexpr detail::named_string<Name> name = {};

  template<typename Derived, bool IsConst, register_type::word_type Val>
    requires(Val < (1ULL << NumBits))
  struct Value {
    using field_type = Derived;

    Value()
      requires(IsConst)
    = default;

    constexpr explicit Value(typename register_type::word_type val)
      requires(!IsConst)
        : m_val {val << Offset} {
      check_(val < (1ULL << NumBits), "invalid value for field");
    }

    struct empty {};

    constexpr auto val() const noexcept -> register_type::word_type {
      if constexpr (IsConst) {
        return Val << Offset;
      } else {
        return m_val;
      }
    }

    constexpr auto natural_val() const noexcept -> register_type::word_type {
      if constexpr (IsConst) {
        return Val;
      } else {
        return m_val >> Offset;
      }
    }

    [[no_unique_address]] std::conditional_t<IsConst, empty, typename register_type::word_type>
        m_val;
  };
};

namespace ops {
template<typename FV1, typename... FVs>
concept belongs_to_same_register =
    field_like<typename FV1::field_type> && (field_like<typename FVs::field_type> && ...)
    && (std::same_as<
            typename field_traits<typename FV1::field_type>::register_type,
            typename field_traits<typename FVs::field_type>::register_type>
        && ...);

template<typename FV1, typename... FVs>
constexpr auto matches_any(
    typename field_traits<typename FV1::field_type>::word_type reg_val,
    FV1 fv1,
    FVs... fvs) noexcept -> bool {
  return field_traits<typename FV1::field_type>::Contains(reg_val, fv1.val())
      || (field_traits<typename FVs::field_type>::Contains(reg_val, fvs.val()) || ...);
}

template<typename FV1, typename... FVs>
constexpr auto matches_all(
    typename field_traits<typename FV1::field_type>::word_type reg_val,
    FV1 fv1,
    FVs... fvs) noexcept -> bool {
  return field_traits<typename FV1::field_type>::Contains(reg_val, fv1.val())
      && (field_traits<typename FVs::field_type>::Contains(reg_val, fvs.val()) && ...);
}

template<typename FV1, typename... FVs>
constexpr auto
Set(typename field_traits<typename FV1::field_type>::word_type reg_val,
    FV1 fv1,
    FVs... fvs) noexcept -> field_traits<typename FV1::field_type>::word_type {
  constexpr auto mask =
      (field_traits<typename FV1::field_type>::mask | ...
       | field_traits<typename FVs::field_type>::mask);
  const auto new_val = (fv1.val() | ... | fvs.val());

  return ktl::SetMaskedBits(reg_val, new_val, mask);
}

template<typename RegisterAccessor>
struct mixin {
  template<typename F>
    requires field_of<F, typename RegisterAccessor::register_type>
  constexpr auto Read(F f) const noexcept {
    const auto reg_val = static_cast<const RegisterAccessor&>(*this).Get();
    constexpr auto mask = field_traits<F>::mask;

    return f.ValFromRaw(reg_val & mask);
  }

  template<typename F>
    requires field_of<F, typename RegisterAccessor::register_type>
  constexpr auto IsSet(F f) const noexcept {
    return Read(f).val() != 0;
  }

  template<typename F>
    requires field_of<F, typename RegisterAccessor::register_type> && field_traits<F>::is_enum
  constexpr auto ReadEnum(F f) const noexcept {
    return field_traits<F>::AsEnum(Read(f).natural_val());
  }

  template<typename FV1, typename... FVs>
    requires ops::belongs_to_same_register<FV1, FVs...>
  constexpr auto MatchesAny(FV1 fv1, FVs... fvs) const noexcept -> bool {
    return ops::matches_any(
        static_cast<const RegisterAccessor&>(*this).Get(),
        std::forward<FV1>(fv1),
        std::forward<FVs>(fvs)...);
  }

  template<typename FV1, typename... FVs>
    requires ops::belongs_to_same_register<FV1, FVs...>
  constexpr auto MatchesAll(FV1 fv1, FVs... fvs) const noexcept -> bool {
    return ops::matches_all(
        static_cast<const RegisterAccessor&>(*this).Get(),
        std::forward<FV1>(fv1),
        std::forward<FVs>(fvs)...);
  }

  template<typename FV1, typename... FVs>
    requires ops::belongs_to_same_register<FV1, FVs...>
  constexpr void Write(FV1 fv1, FVs... fvs) noexcept {
    static_cast<RegisterAccessor&>(*this).Set(
        ops::Set(0, std::forward<FV1>(fv1), std::forward<FVs>(fvs)...));
  }

  template<typename FV1, typename... FVs>
    requires ops::belongs_to_same_register<FV1, FVs...>
  constexpr void Modify(FV1 fv1, FVs... fvs) noexcept {
    static_cast<RegisterAccessor&>(*this).Set(ops::Set(
        static_cast<RegisterAccessor&>(*this).Get(),
        std::forward<FV1>(fv1),
        std::forward<FVs>(fvs)...));
  }

  template<typename FV1, typename... FVs>
    requires ops::belongs_to_same_register<FV1, FVs...>
  constexpr void ModifyNoRead(auto original_val, FV1 fv1, FVs... fvs) noexcept {
    static_cast<RegisterAccessor&>(*this).Set(
        ops::Set(original_val, std::forward<FV1>(fv1), std::forward<FVs>(fvs)...));
  }

  constexpr auto Extract() const noexcept;
};
}  // namespace ops

template<typename FV1, typename FV2>
  requires ops::belongs_to_same_register<FV1, FV2>
    && std::same_as<typename FV1::field_type, typename FV2::field_type>
constexpr auto operator==(const FV1& fv1, const FV2& fv2) noexcept -> bool {
  return fv1.val() == fv2.val();
}

template<typename FV1, typename FV2>
  requires ops::belongs_to_same_register<FV1, FV2>
    && std::same_as<typename FV1::field_type, typename FV2::field_type>
constexpr auto operator<=>(const FV1& fv1, const FV2& fv2) noexcept {
  return fv1.val() <=> fv2.val();
}

template<auto Register>
class LocalCopyRegister: public ops::mixin<LocalCopyRegister<Register>> {
 public:
  using register_type = std::decay_t<decltype(Register)>;
  using word_type = register_type::word_type;

  static_assert(register_like<register_type>);

  LocalCopyRegister() = default;

  constexpr explicit LocalCopyRegister(word_type val) : m_val {val} {}

  constexpr auto Get() const noexcept -> word_type {
    return m_val;
  }

  constexpr void Set(word_type val) noexcept {
    m_val = val;
  }

  friend auto operator==(const LocalCopyRegister&, const LocalCopyRegister&) noexcept
      -> bool = default;
  friend auto operator<=>(const LocalCopyRegister&, const LocalCopyRegister&) noexcept = default;

 private:
  word_type m_val = {};
};

template<auto Register>
class MemoryMappedRegister: public ops::mixin<MemoryMappedRegister<Register>> {
 public:
  using register_type = std::decay_t<decltype(Register)>;
  using word_type = register_type::word_type;

  static_assert(register_like<register_type>);

  MemoryMappedRegister() = delete;
  MemoryMappedRegister(std::nullptr_t) = delete;

  explicit constexpr MemoryMappedRegister(ktl::not_null<word_type*> reg_address) :
      m_reg_address {reg_address} {}

  explicit constexpr MemoryMappedRegister(uintptr_t reg_address) :
      MemoryMappedRegister {std::bit_cast<word_type*>(reg_address)} {}

  auto Get() const noexcept -> word_type {
    return *m_reg_address;
  }

  void Set(word_type val) noexcept {
    *m_reg_address = val;
  }

  friend auto operator==(const MemoryMappedRegister&, const MemoryMappedRegister&) noexcept
      -> bool = default;
  friend auto
  operator<=>(const MemoryMappedRegister&, const MemoryMappedRegister&) noexcept = default;

 private:
  volatile word_type* m_reg_address;
};

template<typename RegisterAccessor>
constexpr auto ops::mixin<RegisterAccessor>::Extract() const noexcept {
  constexpr typename RegisterAccessor::register_type Register = {};
  return LocalCopyRegister<Register> {static_cast<const RegisterAccessor&>(*this).Get()};
}

template<auto Field>
using enum_t = field_traits<std::decay_t<decltype(Field)>>::enum_type;
}  // namespace mei::registers

namespace ktl::fmt {
template<typename CharT, typename FV>
  requires mei::registers::ops::belongs_to_same_register<FV>
struct formatter<CharT, FV> {
  template<typename FormatContext, typename FmtSpec>
    requires std::same_as<CharT, typename FormatContext::char_type>
  constexpr auto format(FormatContext& ctx, const FmtSpec& fmt_spec, const FV& fv) noexcept
      -> expected<bool, Error> {
    using field_traits = mei::registers::field_traits<typename FV::field_type>;
    constexpr auto name = field_traits::field_type::name.view();

    Try(r, ctx.template Format<"{}: [">(std::in_place, name));
    if (!r)
      return false;

    auto val = fv.natural_val();
    if constexpr (field_traits::is_enum) {
      auto str = field_traits::EnumStr(val);
      if (str) {
        TryA(r, ctx.template Format<"{}">(std::in_place, *str));
      } else {
        TryA(r, ctx.template Format<"{}">(std::in_place, val));
      }
    } else {
      TryA(r, ctx.template Format<"{}">(std::in_place, val));
    }
    if (!r)
      return false;

    return ctx.template Format<"]">(std::in_place, fmt_spec);
  }
};

template<typename CharT, auto Register>
struct formatter<CharT, mei::registers::LocalCopyRegister<Register>> {
  template<typename FormatContext, typename FmtSpec>
    requires std::same_as<CharT, typename FormatContext::char_type>
  constexpr auto format(
      FormatContext& ctx,
      const FmtSpec& /* fmt_spec */,
      const mei::registers::LocalCopyRegister<Register>& reg_val) noexcept
      -> expected<bool, Error> {
    using register_type = mei::registers::LocalCopyRegister<Register>::register_type;
    constexpr auto name = register_type::name.view();
    constexpr typename register_type::field_types fields {};

    Try(r,
        ctx.template Format<"{}: {{ {}">(std::in_place, name, reg_val.Read(std::get<0>(fields))));
    if (!r)
      return false;

    TryA(r, format_impl<1>(ctx, reg_val, fields));
    if (!r)
      return false;

    return ctx.template Format<" }}">(std::in_place);
  }

  template<usize I, typename FormatContext, typename... Fields>
  constexpr auto format_impl(
      FormatContext& ctx,
      const mei::registers::LocalCopyRegister<Register>& reg_val,
      const std::tuple<Fields...>& fields) noexcept -> expected<bool, Error> {
    if constexpr (I == sizeof...(Fields)) {
      return false;
    } else if constexpr (I == sizeof...(Fields) - 1) {
      return ctx.template Format<", {}">(std::in_place, reg_val.Read(std::get<I>(fields)));
    } else {
      Try(r, ctx.template Format<", {}">(std::in_place, reg_val.Read(std::get<I>(fields))));
      if (!r)
        return false;
      return format_impl<I + 1>(ctx, reg_val, fields);
    }
  }
};
}  // namespace ktl::fmt

#define DEFINE_SYSTEM_REGISTER(name, regtype, regname_str) \
  namespace defs { \
  struct name: ::mei::registers::ops::mixin<name> { \
    using register_type = regtype; \
    using word_type = register_type::word_type; \
\
    [[nodiscard]] auto Get() const noexcept -> word_type { \
      word_type res; \
      asm volatile("MRS %[res], " regname_str "\n" : [res] "=r"(res)); \
      return res; \
    } \
\
    friend auto operator==(const name&, const name&) noexcept -> bool { \
      return true; \
    } \
    friend auto operator<=>(const name&, const name&) noexcept { \
      return std::strong_ordering::equivalent; \
    } \
\
    void Set(word_type newval) noexcept { \
      asm volatile("MSR " regname_str ", %[newval]\n" : : [newval] "r"(newval)); \
    } \
  }; \
  } \
  inline defs::name CONCAT(name, _sys)
