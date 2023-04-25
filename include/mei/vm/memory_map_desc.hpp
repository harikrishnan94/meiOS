#pragma once

#include <variant>

#include <ktl/int.hpp>

#include "mei/vm/address.hpp"

namespace mei::vm {
enum class MemoryKind {
  // DRAM memory: always cache-able.
  Normal,
  // Device (Peripherals) memory: always non cache-able.
  Device
};

// Access Permissions of a Memory Map
struct AccessPermissions {
  // EL0
  ktl::u8 el0_read : 1;
  ktl::u8 el0_write : 1;
  ktl::u8 el0_execute : 1;

  // EL1
  ktl::u8 el1_read : 1;
  ktl::u8 el1_write : 1;
  ktl::u8 el1_execute : 1;

  static constexpr auto NormalMemoryDefault() noexcept -> AccessPermissions {
    AccessPermissions access_perms = {};
    access_perms.el1_read = access_perms.el1_write = true;
    return access_perms;
  }

  static constexpr auto DeviceMemoryDefault() noexcept -> AccessPermissions {
    AccessPermissions access_perms = {};
    access_perms.el1_read = access_perms.el1_write = true;
    return access_perms;
  }

  static constexpr auto UserMemoryDefault() noexcept -> AccessPermissions {
    auto access_perms = NormalMemoryDefault();
    access_perms.el0_read = access_perms.el0_write = true;
    return access_perms;
  }
};

// Describes a phy_addr -> virt_addr mapping of `num_pages` page count.
struct MapDesc {
  // Page Aligned
  PhysicalAddress phy_addr;
  // Page Aligned
  VirtualAddress virt_addr;
  // `num_pages` is # granule_size'd pages in the mapping.
  ktl::usize num_pages;
  AccessPermissions access_perms;
};

struct MemoryMap {
  // Normal (DRAM) Memory Region is always cache-able.
  // Device Memory Region is always non cache-able.
  MemoryKind kind;
  MapDesc desc;
};
}  // namespace mei::vm