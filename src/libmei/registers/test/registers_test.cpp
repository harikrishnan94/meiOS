#include "generated/test_regs.h"

auto main() -> int {
  using namespace mei::registers;
  using namespace mei::test::test_regs;

  auto sanity_test = [](auto& desc) {
    desc.Set(100);
    check_(desc.Get() == 100, "");

    desc.Modify(
        STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(0xFFFF),
        STAGE1_TABLE_DESCRIPTOR.TYPE.Table);

    check_(
        desc.Read(STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR)
            == STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(0xFFFF),
        "");
    check_(desc.Read(STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR).natural_val() == 0xFFFF, "");
    check_(desc.IsSet(STAGE1_TABLE_DESCRIPTOR.TYPE), "");
    check_(
        desc.ReadEnum(STAGE1_TABLE_DESCRIPTOR.TYPE) == enum_t<STAGE1_TABLE_DESCRIPTOR.TYPE>::Table,
        "");

    desc.Modify(STAGE1_TABLE_DESCRIPTOR.TYPE.Block);
    check_(!desc.IsSet(STAGE1_TABLE_DESCRIPTOR.TYPE), "");

    check_(
        desc.MatchesAny(
            STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(0xFFFF),
            STAGE1_TABLE_DESCRIPTOR.TYPE.Table),
        "");
    check_(
        !desc.MatchesAll(
            STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(0xFFFF),
            STAGE1_TABLE_DESCRIPTOR.TYPE.Table),
        "");

    desc.Modify(STAGE1_TABLE_DESCRIPTOR.TYPE.Table);
    check_(
        desc.MatchesAll(
            STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(0xFFFF),
            STAGE1_TABLE_DESCRIPTOR.TYPE.Table),
        "");
  };

  [[maybe_unused]] static constinit auto local_reg = [&] {
    LocalCopyRegister<STAGE1_TABLE_DESCRIPTOR> desc {0};
    sanity_test(desc);

    return desc.Get();
  }();

  // MemoryMappedRegister
  {
    u64 reg_val = 0;
    MemoryMappedRegister<STAGE1_TABLE_DESCRIPTOR> desc {&reg_val};
    sanity_test(desc);
  }
  return 0;
}
