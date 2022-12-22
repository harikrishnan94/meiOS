use aarch64_cpu::registers::CNTP_CTL_EL0;
use spin::mutex::Mutex;
use tock_registers::interfaces::Readable;

use crate::{
    exception::ExceptionContext,
    gpio::{read_mmio_reg, write_mmio_reg},
};

pub(crate) const CNTP_EL0: u64 = 0x40000040;
pub(crate) const CNTP_STATUS_EL0: u64 = 0x40000060;

const IRQ_BASE_ADDR: u64 = 0x3F000000;
const IRQ_BASIC_PENDING: u64 = IRQ_BASE_ADDR + 0xB200;
const ENABLE_IRQS_1: u64 = IRQ_BASE_ADDR + 0xB210;
const ENABLE_IRQS_2: u64 = IRQ_BASE_ADDR + 0xB214;
const ENABLE_BASIC_IRQS: u64 = IRQ_BASE_ADDR + 0xB218;
const DISABLE_IRQS_1: u64 = IRQ_BASE_ADDR + 0xB21C;
const DISABLE_IRQS_2: u64 = IRQ_BASE_ADDR + 0xB220;
const DISABLE_BASIC_IRQS: u64 = IRQ_BASE_ADDR + 0xB224;

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
pub unsafe fn init_gic() {
    write_mmio_reg(DISABLE_IRQS_1, 0xffffffffu32);
    write_mmio_reg(DISABLE_IRQS_2, 0xffffffffu32);
    write_mmio_reg(DISABLE_BASIC_IRQS, 0xffffffffu32);
}

pub(crate) fn register_interrupt_handler(irq_hand: &'static dyn IRQHandler) {
    let irq_num = irq_hand.get_irq_pending_bit_num() as usize;
    REGISTERED_IRQ_HANDLERS.lock()[irq_num] = IRQHandlerEntry::new(irq_hand);
}

fn is_timer_irq() -> bool {
    unsafe {
        read_mmio_reg::<u64>(CNTP_STATUS_EL0) & (1 << 1) != 0
            && CNTP_CTL_EL0.is_set(CNTP_CTL_EL0::ISTATUS)
    }
}

pub(crate) fn dispatch_peripheral_irq(ec: &mut ExceptionContext) -> bool {
    let irq_pending = unsafe { read_mmio_reg::<u32>(IRQ_BASIC_PENDING) };
    let mut handled = false;

    for i in 0..31 {
        if (irq_pending & (1u32 << i)) != 0 {
            if let Some(handler) = REGISTERED_IRQ_HANDLERS.lock()[i].0 {
                handler.handle(ec);
                handled = true;
            }
        }
    }

    if is_timer_irq() {
        REGISTERED_IRQ_HANDLERS.lock()[0]
            .0
            .as_ref()
            .unwrap()
            .handle(ec);
        handled = true
    }
    handled
}

pub(crate) unsafe fn enable_irq(irq_num: IRQNum) {
    if irq_num < 8 {
        write_mmio_reg(ENABLE_BASIC_IRQS, 1u32 << irq_num);
    } else if irq_num < 32 {
        write_mmio_reg(ENABLE_IRQS_1, 1u32 << irq_num);
    } else {
        let irq_num = irq_num - 32;
        write_mmio_reg(ENABLE_IRQS_2, 1u32 << irq_num);
    }
}
