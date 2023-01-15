#pragma once

#include "field.h"
#include "register.h"

namespace mei::registers {
namespace dtl {
template <register_t R>
struct any_register_value_modifier {
  using Register = R;
  using ResultType = INTT(Register);

  auto Modify(ResultType /*v*/) -> ResultType;
};
}  // namespace dtl

// clang-format off
template <typename RVM, typename Register>
concept register_value_modifier = register_t<Register> && requires(RVM m)
// clang-format on
{
  requires std::same_as<REG(RVM), Register>;
  { m.Modify(std::declval<INTT(Register)>()) } -> std::same_as<typename RVM::ResultType>;
};

// clang-format off
template <typename RS>
concept register_storage = requires(RS &&rs)
// clang-format on
{
  requires register_t<REG(RS)>;
  { rs.Get() } -> std::same_as<REG_INTT(RS)>;
  { rs.Set(std::declval<REG_INTT(RS)>()) } -> std::same_as<void>;
};

template <typename RS, typename R>
concept register_storage_for = register_storage<RS> && register_t<R> && std::same_as<REG(RS), R>;

template <register_t TRegister>
class InMemoryRegister {
 public:
  using Register = TRegister;
  using IntType = INTT(Register);

  constexpr explicit InMemoryRegister(IntType val) : m_val(val) {}

  [[nodiscard]] constexpr auto Get() const -> IntType { return m_val; }

  constexpr void Set(IntType val) { m_val = val; }

 private:
  IntType m_val;
};

template <register_t TRegister>
class MemoryMappedRegister {
 public:
  using Register = TRegister;
  using IntType = INTT(Register);

  explicit MemoryMappedRegister(IntType *val) : m_val(val) {}

  [[nodiscard]] auto Get() const -> IntType { return *m_val; }

  void Set(IntType val) { *m_val = val; }

 private:
  volatile IntType *m_val;
};

template <register_t R>
struct RegisterValueModification {
  using Register = R;
  using ResultType = INTT(Register);

  RegisterValueModification() = default;

  constexpr explicit RegisterValueModification(field_value auto val)
      : m_clr_mask(decltype(val)::Mask), m_update(val.ShiftedVal()) {}

  constexpr void Add(const RegisterValueModification &rhs) {
    m_clr_mask |= rhs.m_clr_mask;
    m_update |= rhs.m_update;
  }

  constexpr void Remove(const RegisterValueModification &rhs) {
    m_clr_mask &= ~rhs.m_clr_mask;
    m_update &= ~rhs.m_clr_mask;
  }

  constexpr void Add(field_value_of<R> auto val) { Add(RegisterValueModification(val)); }
  constexpr void Remove(field_value_of<R> auto val) { Remove(RegisterValueModification(val)); }

  [[nodiscard]] constexpr auto Modify(ResultType oldval) const -> ResultType {
    return (oldval & ~m_clr_mask) | m_update;
  }

 private:
  ResultType m_clr_mask = {};
  ResultType m_update = {};
};

template <field_value FV>
RegisterValueModification(FV) -> RegisterValueModification<REG(FV)>;

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

template <field_value FV>
constexpr auto operator+(const RegisterValueModification<REG(FV)> &lhs, FV rhs)
    -> RegisterValueModification<REG(FV)> {
  auto res = lhs;
  res.Add(rhs);
  return res;
}

template <field_value FV>
constexpr auto operator+=(RegisterValueModification<REG(FV)> &lhs, FV rhs)
    -> RegisterValueModification<REG(FV)> & {
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

template <field_value FV1, field_value FV2>
  requires(std::same_as<REG(FV1), REG(FV2)>)
constexpr auto operator+(FV1 lhs, FV2 rhs) -> RegisterValueModification<REG(FV1)> {
  RegisterValueModification<REG(FV1)> res{lhs};
  res.Add(rhs);
  return res;
}

template <field_value FV>
constexpr auto operator+(FV lhs, const RegisterValueModification<REG(FV)> &rhs)
    -> RegisterValueModification<REG(FV)> {
  RegisterValueModification<REG(FV)> res{lhs};
  res.Add(rhs);
  return res;
}

// Creates a ResultValueModification to be used to remove fields from the RegisterValueModification
// set.
template <field F>
static constexpr auto RM = RegisterValueModification{typename F::Value(0)};

template <field F>
constexpr auto Read(const register_storage_for<REG(F)> auto &rs) -> REG_INTT(F) {
  return (rs.Get() & MaskFor<F>) >> F::Offset;
}

template <enum_field F>
constexpr auto ReadEnum(const register_storage_for<REG(F)> auto &rs) -> typename F::Enum {
  return static_cast<typename F::Enum>(Read<F>(rs));
}

constexpr void Modify(
    register_storage auto &rs,
    const register_value_modifier<REG(std::decay_t<decltype(rs)>)> auto &modifier) {
  rs.Set(modifier.Modify(rs.Get()));
}

constexpr void ModifyNoRead(
    register_storage auto &rs,
    const register_value_modifier<REG(std::decay_t<decltype(rs)>)> auto &modifier) {
  rs.Set(modifier.Modify(0));
}

template <register_storage RS, register_value_modifier<REG(RS)> RVM>
constexpr auto operator|=(RS &rs, const RVM &modifier) -> RS & {
  Modify(rs, modifier);
  return rs;
}

template <register_storage RS>
class NOREAD {
 public:
  constexpr NOREAD(RS &rs) : m_rs(rs) {}

  template <register_value_modifier<REG(RS)> RVM>
  constexpr auto operator|=(const RVM &modifier) -> RS & {
    ModifyNoRead(m_rs, modifier);
    return m_rs;
  }

 private:
  RS &m_rs;
};
}  // namespace mei::registers

#define DEFINE_SYSTEM_REGISTER(name, TRegister, regname_str)                     \
  struct Storage {                                                               \
    using Register = TRegister;                                                  \
    using IntType = INTT(Register);                                              \
                                                                                 \
    [[nodiscard]] auto Get() const -> IntType {                                  \
      IntType res;                                                               \
      asm volatile("MRS %[res], " regname_str "\n" : [res] "=r"(res));           \
      return res;                                                                \
    }                                                                            \
                                                                                 \
    void Set(IntType newval) const {                                             \
      asm volatile("MSR " regname_str ", %[newval]\n" : : [newval] "r"(newval)); \
    }                                                                            \
  } inline const name
