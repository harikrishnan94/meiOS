#![macro_use]
extern crate macros;

use aarch64_cpu::registers::*;
use core::{
    arch::{asm, global_asm},
    fmt,
};
use macros::exception_handler;
use tock_registers::{
    interfaces::{Readable, Writeable},
    registers::InMemoryRegister,
};

use crate::{gic::dispatch_peripheral_irq, println};

global_asm!(include_str!("../asm/rpi3/exception.s"));

mod daifbits {
    pub const IRQ_ENABLE: u8 = 0b0010;
    pub const IRQ_DISABLE: u8 = 0b0000;
}

/// .
///
/// # Safety
///
/// Enables Asynchronous interrupts
pub unsafe fn enable_irq() {
    asm!(
        "msr DAIFClr, {arg}",
        arg = const daifbits::IRQ_ENABLE,
        options(nomem, nostack, preserves_flags)
    );
}

/// .
///
/// # Safety
///
/// Disables Asynchronous interrupts
pub unsafe fn disable_irq() {
    asm!(
        "msr DAIFClr, {arg}",
        arg = const daifbits::IRQ_DISABLE,
        options(nomem, nostack, preserves_flags)
    );
}

/// Initialized by ASM
#[no_mangle]
static VECTOR_TABLE_BASE_ADDR: u64 = 0;

/// .
///
/// # Safety
///
/// Loads vector_table address and stores in VBAR_EL1, setup exception handlers
pub unsafe fn handler_init() {
    let vt_base = core::ptr::read_volatile(&VECTOR_TABLE_BASE_ADDR);
    println!("Loaded Exception vector table from 0x{vt_base:x}");
    VBAR_EL1.set(vt_base);
}

/// Wrapper structs for memory copies of registers.
#[repr(transparent)]
struct SpsrEL1(InMemoryRegister<u64, SPSR_EL1::Register>);
#[repr(transparent)]
struct EsrEL1(InMemoryRegister<u64, ESR_EL1::Register>);

/// The exception context as it is stored on the stack on exception entry.
#[repr(C)]
pub(crate) struct ExceptionContext {
    /// General Purpose Registers.
    gpr: [u64; 30],

    /// The link register, aka x30.
    lr: u64,

    /// Exception link register. The program counter at the time the exception happened.
    elr_el1: u64,

    /// Saved program status.
    spsr_el1: SpsrEL1,

    /// Exception syndrome register.
    esr_el1: EsrEL1,
}

fn default_handler(funcname: &str, ec: &mut ExceptionContext) {
    println!("Unhandled CPU Exception({funcname}): {ec}");
}

#[exception_handler]
fn current_el_sp0_sync(ec: &mut ExceptionContext) {
    default_handler("current_el_sp0_sync", ec);
}

#[exception_handler]
fn current_el_sp0_irq(ec: &mut ExceptionContext) {
    default_handler("current_el_sp0_irq", ec);
}

#[exception_handler]
fn current_el_sp0_fiq(ec: &mut ExceptionContext) {
    default_handler("current_el_sp0_fiq", ec);
}

#[exception_handler]
fn current_el_sp0_serror(ec: &mut ExceptionContext) {
    default_handler("current_el_sp0_serror", ec);
}

#[exception_handler]
fn current_el_spn_sync(ec: &mut ExceptionContext) {
    default_handler("current_el_spn_sync", ec);
}

#[exception_handler]
fn current_el_spn_irq(ec: &mut ExceptionContext) {
    if !dispatch_peripheral_irq(ec) {
        default_handler("current_el_spn_irq", ec);
    }
}

#[exception_handler]
fn current_el_spn_fiq(ec: &mut ExceptionContext) {
    default_handler("current_el_spn_fiq", ec);
}

#[exception_handler]
fn current_el_spn_serror(ec: &mut ExceptionContext) {
    default_handler("current_el_spn_serror", ec);
}

#[exception_handler]
fn lower_el_aarch64_sync(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch64_sync", ec);
}

#[exception_handler]
fn lower_el_aarch64_irq(ec: &mut ExceptionContext) {
    if !dispatch_peripheral_irq(ec) {
        default_handler("lower_el_aarch64_irq", ec);
    }
}

#[exception_handler]
fn lower_el_aarch64_fiq(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch64_fiq", ec);
}

#[exception_handler]
fn lower_el_aarch64_serror(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch64_serror", ec);
}

#[exception_handler]
fn lower_el_aarch32_sync(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch32_sync", ec);
}

#[exception_handler]
fn lower_el_aarch32_irq(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch32_irq", ec);
}

#[exception_handler]
fn lower_el_aarch32_fiq(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch32_fiq", ec);
}

#[exception_handler]
fn lower_el_aarch32_serror(ec: &mut ExceptionContext) {
    default_handler("lower_el_aarch32_serror", ec);
}

//------------------------------------------------------------------------------
// Misc
//------------------------------------------------------------------------------

/// Human readable SPSR_EL1.
#[rustfmt::skip]
impl fmt::Display for SpsrEL1 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Raw value.
        writeln!(f, "SPSR_EL1: {:#010x}", self.0.get())?;

        let to_flag_str = |x| -> _ {
            if x { "Set" } else { "Not set" }
         };

        writeln!(f, "      Flags:")?;
        writeln!(f, "            Negative (N): {}", to_flag_str(self.0.is_set(SPSR_EL1::N)))?;
        writeln!(f, "            Zero     (Z): {}", to_flag_str(self.0.is_set(SPSR_EL1::Z)))?;
        writeln!(f, "            Carry    (C): {}", to_flag_str(self.0.is_set(SPSR_EL1::C)))?;
        writeln!(f, "            Overflow (V): {}", to_flag_str(self.0.is_set(SPSR_EL1::V)))?;

        let to_mask_str = |x| -> _ {
            if x { "Masked" } else { "Unmasked" }
        };

        writeln!(f, "      Exception handling state:")?;
        writeln!(f, "            Debug  (D): {}", to_mask_str(self.0.is_set(SPSR_EL1::D)))?;
        writeln!(f, "            SError (A): {}", to_mask_str(self.0.is_set(SPSR_EL1::A)))?;
        writeln!(f, "            IRQ    (I): {}", to_mask_str(self.0.is_set(SPSR_EL1::I)))?;
        writeln!(f, "            FIQ    (F): {}", to_mask_str(self.0.is_set(SPSR_EL1::F)))?;

        write!(f, "      Illegal Execution State (IL): {}",
            to_flag_str(self.0.is_set(SPSR_EL1::IL))
        )
    }
}

impl EsrEL1 {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.0.read_as_enum(ESR_EL1::EC)
    }
}

/// Human readable ESR_EL1.
#[rustfmt::skip]
impl fmt::Display for EsrEL1 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Raw print of whole register.
        writeln!(f, "ESR_EL1: {:#010x}", self.0.get())?;

        // Raw print of exception class.
        write!(f, "      Exception Class         (EC) : {:#x}", self.0.read(ESR_EL1::EC))?;

        // Exception class.
        let ec_translation = match self.exception_class() {
            Some(ESR_EL1::EC::Value::DataAbortCurrentEL) => "Data Abort, current EL",
            _ => "N/A",
        };
        writeln!(f, " - {ec_translation}")?;

        // Raw print of instruction specific syndrome.
        write!(f, "      Instr Specific Syndrome (ISS): {:#x}", self.0.read(ESR_EL1::ISS))
    }
}

impl ExceptionContext {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.esr_el1.exception_class()
    }

    #[inline(always)]
    fn fault_address_valid(&self) -> bool {
        use ESR_EL1::EC::Value::*;

        match self.exception_class() {
            None => false,
            Some(ec) => matches!(
                ec,
                InstrAbortLowerEL
                    | InstrAbortCurrentEL
                    | PCAlignmentFault
                    | DataAbortLowerEL
                    | DataAbortCurrentEL
                    | WatchpointLowerEL
                    | WatchpointCurrentEL
            ),
        }
    }
}

/// Human readable print of the exception context.
impl fmt::Display for ExceptionContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.esr_el1)?;

        if self.fault_address_valid() {
            writeln!(f, "FAR_EL1: {:#018x}", FAR_EL1.get() as usize)?;
        }

        writeln!(f, "{}", self.spsr_el1)?;
        writeln!(f, "ELR_EL1: {:#018x}", self.elr_el1)?;
        writeln!(f)?;
        writeln!(f, "General purpose register:")?;

        #[rustfmt::skip]
        let alternating = |x| -> _ {
            if x % 2 == 0 { "   " } else { "\n" }
        };

        // Print two registers per line.
        for (i, reg) in self.gpr.iter().enumerate() {
            write!(f, "      x{: <2}: {: >#018x}{}", i, reg, alternating(i))?;
        }
        write!(f, "      lr : {:#018x}", self.lr)
    }
}
