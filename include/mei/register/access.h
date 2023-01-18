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
template <typename FS, typename Register>
concept field_set = register_t<Register> && requires(FS s)
// clang-format on
{
  requires std::same_as<REG(FS), Register>;
  { s.Modify(std::declval<INTT(Register)>()) } -> std::same_as<typename FS::ResultType>;
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

  [[nodiscard]] auto Get() const -> IntType { return m_val; }

  void Set(IntType val) { m_val = val; }

 private:
  volatile IntType m_val;
};

template <register_t R>
struct FieldSet {
  using Register = R;
  using ResultType = INTT(Register);

  FieldSet() = default;

  constexpr explicit FieldSet(field_value auto val)
      : m_clr_mask(decltype(val)::Mask), m_update(val.ShiftedVal()) {}

  constexpr void Add(const FieldSet &rhs) {
    m_clr_mask |= rhs.m_clr_mask;
    m_update |= rhs.m_update;
  }

  constexpr void Remove(const FieldSet &rhs) {
    m_clr_mask &= ~rhs.m_clr_mask;
    m_update &= ~rhs.m_clr_mask;
  }

  constexpr void Add(field_value_of<R> auto val) { Add(FieldSet(val)); }
  constexpr void Remove(field_value_of<R> auto val) { Remove(FieldSet(val)); }

  [[nodiscard]] constexpr auto Modify(ResultType oldval) const -> ResultType {
    return (oldval & ~m_clr_mask) | m_update;
  }

 private:
  ResultType m_clr_mask = {};
  ResultType m_update = {};
};

template <field_value FV>
FieldSet(FV) -> FieldSet<REG(FV)>;

template <register_t Register>
constexpr auto operator+(const FieldSet<Register> &lhs, const FieldSet<Register> &rhs)
    -> FieldSet<Register> {
  auto res = lhs;
  res.Add(rhs);
  return res;
}

template <register_t Register>
constexpr auto operator-(const FieldSet<Register> &lhs, const FieldSet<Register> &rhs)
    -> FieldSet<Register> {
  auto res = lhs;
  res.Remove(rhs);
  return res;
}

template <field_value FV>
constexpr auto operator+(const FieldSet<REG(FV)> &lhs, FV rhs) -> FieldSet<REG(FV)> {
  auto res = lhs;
  res.Add(rhs);
  return res;
}

template <field_value FV>
constexpr auto operator+=(FieldSet<REG(FV)> &lhs, FV rhs) -> FieldSet<REG(FV)> & {
  lhs.Add(rhs);
  return lhs;
}

template <register_t Register>
constexpr auto operator-=(FieldSet<Register> &lhs, const FieldSet<Register> &rhs)
    -> FieldSet<Register> & {
  lhs.Remove(rhs);
  return lhs;
}

template <field_value FV1, field_value FV2>
  requires(std::same_as<REG(FV1), REG(FV2)>)
constexpr auto operator+(FV1 lhs, FV2 rhs) -> FieldSet<REG(FV1)> {
  FieldSet<REG(FV1)> res{lhs};
  res.Add(rhs);
  return res;
}

template <field_value FV>
constexpr auto operator+(FV lhs, const FieldSet<REG(FV)> &rhs) -> FieldSet<REG(FV)> {
  FieldSet<REG(FV)> res{lhs};
  res.Add(rhs);
  return res;
}

// Creates a ResultValueModification to be used to remove fields from the RegisterValueModification
// set.
template <field F>
static constexpr auto RM = FieldSet{typename F::Value(0)};

template <field F>
constexpr auto Read(const register_storage_for<REG(F)> auto &rs) -> REG_INTT(F) {
  return (rs.Get() & MaskFor<F>) >> F::Offset;
}

template <enum_field F>
constexpr auto ReadEnum(const register_storage_for<REG(F)> auto &rs) -> typename F::Enum {
  return static_cast<typename F::Enum>(Read<F>(rs));
}

constexpr void Modify(register_storage auto &rs,
                      const field_set<REG(std::decay_t<decltype(rs)>)> auto &fs) {
  rs.Set(fs.Modify(rs.Get()));
}

constexpr void ModifyNoRead(register_storage auto &rs,
                            const field_set<REG(std::decay_t<decltype(rs)>)> auto &fs) {
  rs.Set(fs.Modify(0));
}

template <register_storage RS, field_set<REG(RS)> FS>
constexpr auto operator|=(RS &rs, const FS &fs) -> RS & {
  Modify(rs, fs);
  return rs;
}

template <register_storage RS>
class NOREAD {
 public:
  constexpr NOREAD(RS &rs) : m_rs(rs) {}

  template <field_set<REG(RS)> FS>
  constexpr auto operator|=(const FS &fs) -> RS & {
    ModifyNoRead(m_rs, fs);
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
