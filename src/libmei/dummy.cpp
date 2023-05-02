#include <mei/vm/address.hpp>
#include <mei/vm/debug.hpp>
#include <mei/vm/translation/table.hpp>

namespace mei::vm::translation {

template<control_like Control>
struct FourKiBGranuleOps {
  using control = Control;

  template<ktl::u32 Level>
  static constexpr auto get_next_level_desc(detail::table_descriptor tdesc) noexcept -> uintptr_t {
    static_assert(Level != num_levels<Control> - 1);
    return tdesc.Read(regs::STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR).get()
        << granule_bits<Control>;
  }

  template<ktl::u32 Level>
  static constexpr void
  set_next_level_desc(detail::table_descriptor& tdesc, uintptr_t next_level_desc) noexcept {
    static_assert(Level != num_levels<Control> - 1);
    tdesc.Modify(regs::STAGE1_TABLE_DESCRIPTOR.NEXT_LEVEL_TABLE_ADDR(next_level_desc));
  }

  template<ktl::u32 Level>
  static constexpr auto get_output_address(detail::block_descriptor bdesc) noexcept -> uintptr_t {
    static_assert(can_have_block_desc_at(Level));
    if constexpr (Level == 1) {
      return bdesc.Read(regs::STAGE1_BLOCK_DESCRIPTOR.OUTPUT_ADDR_1GiB).get()
          << ktl::at(start_bit_for_level<Control>, Level);
    } else if constexpr (Level == 2) {
      return bdesc.Read(regs::STAGE1_BLOCK_DESCRIPTOR.OUTPUT_ADDR_2MiB).get()
          << ktl::at(start_bit_for_level<Control>, Level);
    } else {
      assert(false);
    }
  }

  template<ktl::u32 Level>
  static constexpr void
  set_output_address(detail::block_descriptor& bdesc, uintptr_t output_address) noexcept {
    static_assert(can_have_block_desc_at(Level));
    output_address >>= ktl::at(start_bit_for_level<Control>, Level);
    if constexpr (Level == 1) {
      bdesc.Modify(regs::STAGE1_BLOCK_DESCRIPTOR.OUTPUT_ADDR_1GiB(output_address));
    } else if constexpr (Level == 2) {
      bdesc.Modify(regs::STAGE1_BLOCK_DESCRIPTOR.OUTPUT_ADDR_2MiB(output_address));
    } else {
      assert(false);
    }
  }

  template<ktl::u32 Level>
  static constexpr auto get_output_address(detail::page_descriptor pdesc) noexcept -> uintptr_t {
    static_assert(Level == num_levels<Control> - 1);
    return pdesc.Read(regs::STAGE1_PAGE_DESCRIPTOR.OUTPUT_ADDR_4KiB).get() << granule_bits<Control>;
  }

  template<ktl::u32 Level>
  static constexpr void
  set_output_address(detail::page_descriptor& pdesc, uintptr_t output_address) noexcept {
    static_assert(Level == num_levels<Control> - 1);
    pdesc.Modify(regs::STAGE1_PAGE_DESCRIPTOR.OUTPUT_ADDR_4KiB(output_address));
  }

  static constexpr auto can_have_block_desc_at(ktl::u32 level) noexcept -> bool {
    return level != 0 && level != num_levels<Control> - 1;
  }
};

using control = Control<
    param::VirtualAddressBits<64>,
    param::VirtualAddressSpaceBits<48>,
    param::TopByteIgnore<true>,
    param::GranuleBits<12>,
    param::MaxBitsPerLevel<9>>;
auto my_virt2phy(const DescriptorTable<control>& root, VirtualAddress vaddr) {
  return Virt2Phy<FourKiBGranuleOps<control>>(root, vaddr);
}
}  // namespace mei::vm::translation
