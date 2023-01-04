use aarch64_cpu::registers::CNTP_CTL_EL0;
use spin::mutex::Mutex;
use tock_registers::interfaces::Readable;

use crate::{
    address::PhysicalAddress,
    error::Result,
    exception::ExceptionContext,
    mimo::{CNTP_STATUS_EL0, MIMORW, PERIPHERAL_IC_BASE},
};

const IRQ_BASIC_PENDING: PhysicalAddress = PERIPHERAL_IC_BASE;
const ENABLE_IRQS_1: PhysicalAddress = PERIPHERAL_IC_BASE + 0x10usize;
const ENABLE_IRQS_2: PhysicalAddress = PERIPHERAL_IC_BASE + 0x14usize;
const ENABLE_BASIC_IRQS: PhysicalAddress = PERIPHERAL_IC_BASE + 0x18usize;
const DISABLE_IRQS_1: PhysicalAddress = PERIPHERAL_IC_BASE + 0x1Cusize;
const DISABLE_IRQS_2: PhysicalAddress = PERIPHERAL_IC_BASE + 0x20usize;
const DISABLE_BASIC_IRQS: PhysicalAddress = PERIPHERAL_IC_BASE + 0x24usize;

pub(crate) type IRQNum = u32;
const MAX_IRQ_NUM: u32 = 64;

/// Core Interrupt Request Handler Trait.
pub(crate) trait IRQHandler: Send + Sync {
    /// Return the interrupt request number associated with the handler
    fn get_irq_pending_bit_num(&self) -> IRQNum;

    /// Handle the interrput
    /// Called when corresponding interrupt pending bit is set.
    fn handle(&self, ec: &mut ExceptionContext);
}

#[derive(Default, Clone, Copy)]
struct IRQHandlerEntry<'a>(Option<&'a dyn IRQHandler>);

impl<'a> IRQHandlerEntry<'a> {
    fn new(irq_hand: &'a dyn IRQHandler) -> Self {
        Self(Some(irq_hand))
    }
}

lazy_static! {
    static ref REGISTERED_IRQ_HANDLERS: Mutex<[IRQHandlerEntry<'static>; MAX_IRQ_NUM as usize]> =
        Mutex::new([IRQHandlerEntry::default(); MAX_IRQ_NUM as usize]);
}

/// .
///
/// # Safety
///
/// Initialize BCM2537 Interrupt controller
pub unsafe fn init_gic() -> Result<()> {
    DISABLE_IRQS_1.write_reg(0xffffffffu32)?;
    DISABLE_IRQS_2.write_reg(0xffffffffu32)?;
    DISABLE_BASIC_IRQS.write_reg(0xffffffffu32)?;
    Ok(())
}

pub(crate) fn register_interrupt_handler(irq_hand: &'static dyn IRQHandler) {
    let irq_num = irq_hand.get_irq_pending_bit_num() as usize;
    REGISTERED_IRQ_HANDLERS.lock()[irq_num] = IRQHandlerEntry::new(irq_hand);
}

fn is_timer_irq() -> Result<bool> {
    Ok(unsafe {
        CNTP_STATUS_EL0.read_reg::<u64>()? & (1 << 1) != 0
            && CNTP_CTL_EL0.is_set(CNTP_CTL_EL0::ISTATUS)
    })
}

pub(crate) fn dispatch_peripheral_irq(ec: &mut ExceptionContext) -> Result<bool> {
    let irq_pending = unsafe { IRQ_BASIC_PENDING.read_reg::<u32>()? };
    let mut handled = false;

    for i in 0..31 {
        if (irq_pending & (1u32 << i)) != 0 {
            if let Some(handler) = REGISTERED_IRQ_HANDLERS.lock()[i].0 {
                handler.handle(ec);
                handled = true;
            }
        }
    }

    if is_timer_irq()? {
        REGISTERED_IRQ_HANDLERS.lock()[0]
            .0
            .as_ref()
            .unwrap()
            .handle(ec);
        handled = true
    }
    Ok(handled)
}

pub(crate) unsafe fn enable_irq(irq_num: IRQNum) -> Result<()> {
    if irq_num < 8 {
        return ENABLE_BASIC_IRQS.write_reg(1u32 << irq_num);
    } else if irq_num < 32 {
        return ENABLE_IRQS_1.write_reg(1u32 << irq_num);
    } else {
        let irq_num = irq_num - 32;
        return ENABLE_IRQS_2.write_reg(1u32 << irq_num);
    }
}
