#include <ktl/static_string.hpp>
#include <ktl/test/platform.hpp>

#include "generated/test_regs.hpp"

using namespace ktl;
using namespace ktl::fmt::literals;

namespace mei::test::test_regs {
DEF_MEM_MAPD_DEV(uart, ktl::u32, 100, {
  DEF_MEM_MAPD_DEV_REG(MAIR_EL1, 0, mair_el1);
  DEF_MEM_MAPD_DEV_REG(test_regs::TCR_EL1, 0, tcr_el1);
});
}

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
    check_(desc.Read(STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR).get() == 0xFFFF, "");
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

  // Format test
  [[maybe_unused]] static constinit auto format_test = [] {
    LocalCopyRegister<TCR_EL1> desc {0};

    static_string<1024> str;

    auto res = "{}"_f.format(str, desc);
    check_(res, "");

    check_(
        str == "TCR_EL1: { TBID1: [0], TBID0: [0], HD: [Disable], HA: [Disable], TBI1: [Used], TBI0: [Used], AS: [ASID8Bits], IPS: [Bits_32], TG1: [0], SH1: [None], ORGN1: [NonCacheable], IRGN1: [NonCacheable], EPD1: [EnableTTBR1Walks], A1: [TTBR0], T1SZ: [0], TG0: [KiB_4], SH0: [None], ORGN0: [NonCacheable], IRGN0: [NonCacheable], EPD0: [EnableTTBR0Walks], T0SZ: [0] }",
        "");

    return 0;
  }();

  // MemoryMapped Device test
  {
    uart::word_type reg[uart::num_words] = {};
    auto* uart_dev = uart::at(&reg[0]);
    check_(!uart_dev->mair_el1.IsSet(MAIR_EL1.Attr0_Device), "");
    check_(!uart_dev->tcr_el1.IsSet(TCR_EL1.HA), "");
  }
  return 0;
}
