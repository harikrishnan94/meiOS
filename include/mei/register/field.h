#pragma once

#include <cassert>
#include <string_view>

#include "mei/bitops.h"
#include "mei/types.h"
#include "register.h"

namespace mei::registers {
// clang-format off
template <typename F>
concept field = requires(F)
// clang-format on
{
  requires register_t<REG(F)>;
  { F::Name } -> dtl::same_as<const char *>;
  { F::Offset } -> dtl::same_as<uint>;
  { F::NumBits } -> dtl::same_as<uint>;
};

// clang-format off
template <typename F>
concept enum_field = field<F> && requires(F)
// clang-format on
{
  requires std::is_enum_v<typename F::Enum>;
  { F::EnumValueToStr(static_cast<REG_INTT(F)>(0)) } -> std::convertible_to<std::string_view>;
};

template <typename F, typename R>
concept field_of = register_t<R> && field<F> && requires(F) { requires std::same_as<R, REG(F)>; };

template <register_t R, field_of<R> Field1, typename... OtherFields>
static constexpr auto AreFieldsBelongToSameRegister =
    ((field_of<R, OtherFields> && ...) &&
     (std::same_as<typename Field1::Register, typename OtherFields::Register> && ...));

template <register_t R, typename... Field>
  requires(field_of<Field, R> && ...)
static constexpr auto OverlappingMaskFor =
    ((bits::CreateMask<typename Field::Register::IntType>(Field::NumBits, Field::Offset)) | ...);

template <register_t R, typename... Field>
  requires(AreFieldsBelongToSameRegister<R, Field...>)
static constexpr bool AreFieldsNonOverlapping = bits::Count(OverlappingMaskFor<R, Field...>) ==
                                                (Field::NumBits + ...);

template <field FirstField, typename... Field>
  requires AreFieldsNonOverlapping<typename FirstField::Register, FirstField, Field...>
static constexpr auto MaskFor =
    OverlappingMaskFor<typename FirstField::Register, FirstField, Field...>;

// clang-format off
template <typename FV>
concept field_value = requires(FV v)
// clang-format on
{
  requires register_t<REG(FV)>;
  requires field<typename FV::Field>;
  { FV::Mask } -> dtl::same_as<REG(FV)::IntType>;
  { v.Val() } -> dtl::same_as<REG(FV)::IntType>;
  { v.ShiftedVal() } -> dtl::same_as<REG(FV)::IntType>;
};

template <typename FV, typename R>
concept field_value_of = register_t<R> && field_value<FV> && std::same_as<REG(FV), R>;

template <register_t R, field_of<R> F>
class FieldValue {
 public:
  using Register = R;
  using Field = F;
  using ResultType = typename Register::IntType;

  static constexpr ResultType Mask = bits::CreateMask<ResultType>(Field::NumBits, Field::Offset);

  constexpr FieldValue(ResultType val) : m_val(val & (Mask >> Field::Offset)) {
    assert(val == m_val >> Offset);
  }

  [[nodiscard]] constexpr auto Val() const -> ResultType { return m_val; }

  [[nodiscard]] constexpr auto ShiftedVal() const -> ResultType { return m_val << Field::Offset; }

  [[nodiscard]] constexpr auto Modify(ResultType oldval) const -> ResultType {
    return (oldval & ~Mask) | ShiftedVal();
  }

 private:
  ResultType m_val;
};

template <dtl::fixed_string TName, register_t TRegister, uint TOffset, uint TNumBits>
  requires((TOffset + TNumBits) <= bits::Count<INTT(TRegister)>(-1))
struct Field {
  using Register = TRegister;
  static constexpr const char *Name = TName.data();
  static constexpr uint Offset = TOffset;
  static constexpr uint NumBits = TNumBits;

  using Value = FieldValue<Register, Field>;
};
}  // namespace mei::registers
