#pragma once

#include "field.h"
#include "register.h"

namespace mei::registers {
namespace dtl {
template <register_t R>
struct any_register_value_modifier {
  using Register = R;
  using ResultType = typename Register::IntType;

  auto Modify(ResultType /*v*/) -> ResultType;
};
}  // namespace dtl

// clang-format off
template <typename RVM, typename Register>
concept register_value_modifier = register_t<Register> && requires(RVM m)
// clang-format on
{
  requires std::same_as<typename RVM::Register, Register>;
  {
    m.Modify(std::declval<typename Register::IntType>())
    } -> std::same_as<typename RVM::ResultType>;
};

// clang-format off
template <typename RS, typename Register>
concept register_storage = register_t<Register> && requires(const RS &ca, RS a)
// clang-format on
{
  requires std::same_as<typename RS::Register, Register>;
  { ca.Get() } -> std::same_as<typename Register::IntType>;
  { a.Set(std::declval<typename Register::IntType>()) } -> std::same_as<void>;
};

// clang-format off
template <typename RA, typename Register>
concept register_accessor = register_t<Register> && register_storage<RA, Register> &&
                            requires(RA a, dtl::any_register_value_modifier<Register> any)
// clang-format on
{
  { a.Modify(any) } -> std::convertible_to<void>;
  { a.ModifyNoRead(any) } -> std::convertible_to<void>;
};

template <register_t Register, register_storage<Register> RS>
struct Accessor : public RS {
  using RS::RS;

  constexpr void Modify(register_value_modifier<Register> auto modifier) {
    RS::Set(modifier.Modify(RS::Get()));
  }

  constexpr void ModifyNoRead(register_value_modifier<Register> auto modifier) {
    RS::Set(modifier.Modify(0));
  }
};

template <register_t TRegister>
class InMemoryStorage {
 public:
  using Register = TRegister;
  using IntType = typename Register::IntType;

  constexpr explicit InMemoryStorage(IntType val) : m_val(val) {}

  [[nodiscard]] constexpr auto Get() const -> IntType { return m_val; }

  constexpr void Set(IntType val) { m_val = val; }

 private:
  IntType m_val;
};

template <register_t TRegister>
class MemoryMappedStorage {
 public:
  using Register = TRegister;
  using IntType = typename Register::IntType;

  explicit MemoryMappedStorage(IntType *val) : m_val(val) {}

  [[nodiscard]] auto Get() const -> IntType { return *m_val; }

  void Set(IntType val) { *m_val = val; }

 private:
  volatile IntType *m_val;
};

template <register_t Register>
using InMemoryRegister = Accessor<Register, InMemoryStorage<Register>>;

template <register_t Register>
using MemoryMappedRegister = Accessor<Register, MemoryMappedStorage<Register>>;

template <register_t R>
struct RegisterValueModification {
  using Register = R;
  using ResultType = typename Register::IntType;

  RegisterValueModification() = default;

  template <uint Offset, uint NumBits>
  constexpr explicit RegisterValueModification(FieldValue<R, Offset, NumBits> val)
      : m_clr_mask(decltype(val)::Mask), m_update(val.ShiftedVal()) {}

  constexpr void Add(const RegisterValueModification &rhs) {
    m_clr_mask |= rhs.m_clr_mask;
    m_update |= rhs.m_update;
  }

  constexpr void Remove(const RegisterValueModification &rhs) {
    m_clr_mask &= ~rhs.m_clr_mask;
    m_update &= ~rhs.m_clr_mask;
  }

  template <uint Offset, uint NumBits>
  constexpr void Add(FieldValue<R, Offset, NumBits> val) {
    Add(RegisterValueModification(val));
  }

  template <uint Offset, uint NumBits>
  constexpr void Remove(FieldValue<R, Offset, NumBits> val) {
    Remove(RegisterValueModification(val));
  }

  [[nodiscard]] constexpr auto Modify(ResultType oldval) const -> ResultType {
    return (oldval & ~m_clr_mask) | m_update;
  }

 private:
  ResultType m_clr_mask = {};
  ResultType m_update = {};
};

template <register_t Register>
constexpr auto operator+(const RegisterValueModification<Register> &lhs,
                         const RegisterValueModification<Register> &rhs)
    -> RegisterValueModification<Register> {
  auto res = lhs;
  res.Add(rhs);
  return res;
}

template <register_t Register>
constexpr auto operator-(const RegisterValueModification<Register> &lhs,
                         const RegisterValueModification<Register> &rhs)
    -> RegisterValueModification<Register> {
  auto res = lhs;
  res.Remove(rhs);
  return res;
}

template <register_t Register, uint Offset, uint NumBits>
constexpr auto operator+(const RegisterValueModification<Register> &lhs,
                         FieldValue<Register, Offset, NumBits> rhs)
    -> RegisterValueModification<Register> {
  auto res = lhs;
  res.Add(rhs);
  return res;
}

template <register_t Register>
constexpr auto operator+=(RegisterValueModification<Register> &lhs,
                          const RegisterValueModification<Register> &rhs)
    -> RegisterValueModification<Register> & {
  lhs.Add(rhs);
  return lhs;
}

template <register_t Register, uint Offset, uint NumBits>
constexpr auto operator+=(RegisterValueModification<Register> &lhs,
                          FieldValue<Register, Offset, NumBits> rhs)
    -> RegisterValueModification<Register> & {
  lhs.Add(rhs);
  return lhs;
}

template <register_t Register>
constexpr auto operator-=(RegisterValueModification<Register> &lhs,
                          const RegisterValueModification<Register> &rhs)
    -> RegisterValueModification<Register> & {
  lhs.Remove(rhs);
  return lhs;
}

template <register_t Register, uint Offset1, uint NumBits1, uint Offset2, uint NumBits2>
constexpr auto operator+(FieldValue<Register, Offset1, NumBits1> lhs,
                         FieldValue<Register, Offset2, NumBits2> rhs)
    -> RegisterValueModification<Register> {
  RegisterValueModification<Register> res{lhs};
  res.Add(rhs);
  return res;
}

template <register_t Register, uint Offset, uint NumBits>
constexpr auto operator+(FieldValue<Register, Offset, NumBits> lhs,
                         const RegisterValueModification<Register> &rhs)
    -> RegisterValueModification<Register> {
  RegisterValueModification<Register> res{lhs};
  res.Add(rhs);
  return res;
}

// Creates a ResultValueModification to be used to remove fields from the RegisterValueModification
// set.
template <field F>
static constexpr auto RM = RegisterValueModification{typename F::Value(0)};

}  // namespace mei::registers

#define DEFINE_SYSTEM_REGISTER(name, TRegister, regname_str)                                     \
  struct {                                                                                       \
    using Register = TRegister;                                                                  \
    using IntType = typename Register::IntType;                                                  \
                                                                                                 \
    [[nodiscard]] auto Get() const -> IntType {                                                  \
      IntType res;                                                                               \
      asm volatile("MRS %[res], " regname_str "\n" : [res] "=r"(res));                           \
      return res;                                                                                \
    }                                                                                            \
                                                                                                 \
    void Set(IntType newval) const {                                                             \
      asm volatile("MSR " regname_str ", %[newval]\n" : : [newval] "r"(newval));                 \
    }                                                                                            \
                                                                                                 \
    void Modify(::mei::registers::register_value_modifier<Register> auto modifier) const {       \
      Set(modifier.Modify(Get()));                                                               \
    }                                                                                            \
                                                                                                 \
    void ModifyNoRead(::mei::registers::register_value_modifier<Register> auto modifier) const { \
      Set(modifier.Modify(0));                                                                   \
    }                                                                                            \
                                                                                                 \
  } inline const name
