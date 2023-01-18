#pragma once

// Entry point for kernel c++ code.
#define MEI_MAIN mei_main

/* MPIDR_EL1's last 2 bits contain the current cpu */
#define CPUID_MASK ((1 << 2) - 1)
#define BOOT_CORE_ID (0)
/* CurrentEL's 2:3 contains the exception level */
#define EL_BITS_OFFSET (2)
/* Hypervisor mode EL is 2 */
#define HYP_MODE_EL (2)