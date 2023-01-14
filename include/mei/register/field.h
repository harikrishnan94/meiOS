#pragma once

#include <cassert>

#include "mei/bitops.h"
#include "mei/types.h"
#include "register.h"

namespace mei::registers {
// clang-format off
template <typename F>
concept field = requires(F)
// clang-format on
{
  requires register_t<typename F::Register>;
  { F::Name } -> dtl::same_as<const char *>;
  { F::Offset } -> dtl::same_as<uint>;
  { F::NumBits } -> dtl::same_as<uint>;
};

template <field Field1, typename... OtherFields>
static constexpr auto all_belong_to_same_register_v =
    ((field<OtherFields> && ...) &&
     (std::same_as<typename Field1::RegisterType, typename OtherFields::RegisterType> && ...));

template <typename... Field>
  requires(field<Field> && ...)
constexpr auto CreateMaskOverlapping() {
  return ((bits::CreateMask<typename Field::RegisterType::IntType>(Field::NumBits, Field::Offset)) |
          ...);
}

template <typename... Field>
  requires(all_belong_to_same_register_v<Field...>)
static constexpr bool are_non_overlapping_v = bits::Count(CreateMaskOverlapping<Field...>()) ==
                                              (Field::NumBits + ...);

template <typename... Field>
  requires are_non_overlapping_v<Field...>
constexpr auto CreateMask() {
  return CreateMaskOverlapping<Field...>();
}

template <register_t R, uint Offset, uint NumBits>
class FieldValue {
 public:
  using Register = R;
  using ResultType = typename Register::IntType;

  static constexpr ResultType Mask = bits::CreateMask<ResultType>(NumBits, Offset);

  constexpr FieldValue(ResultType val) : m_val(val & (Mask >> Offset)) {
    assert(val == m_val >> Offset);
  }

  [[nodiscard]] constexpr auto Val() const -> ResultType { return m_val; }

  [[nodiscard]] constexpr auto ShiftedVal() const -> ResultType { return m_val << Offset; }

  [[nodiscard]] constexpr auto Modify(ResultType oldval) const -> ResultType {
    return (oldval & ~Mask) | ShiftedVal();
  }

 private:
  ResultType m_val;
};

template <dtl::fixed_string TName, register_t TRegister, uint TOffset, uint TNumBits>
  requires((TOffset + TNumBits) <= bits::Count<typename TRegister::IntType>(-1))
struct Field {
  using Register = TRegister;
  static constexpr const char *Name = TName.data();
  static constexpr uint Offset = TOffset;
  static constexpr uint NumBits = TNumBits;

  using Value = FieldValue<Register, Offset, NumBits>;
};
}  // namespace mei::registers
