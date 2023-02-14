use core::time::Duration;

use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_TVAL_EL0};
use macros::ctor;
use spin::Mutex;
use tock_registers::interfaces::Writeable;

use crate::{
    address_map::CNTP_EL0,
    arch::exception::ExceptionContext,
    arch::gic::{register_interrupt_handler, IRQHandler, IRQNum},
    mimo::MIMORW,
    println,
};

/// Will be initialized by ASM (boot.s)
#[no_mangle]
static TIMER_FREQ: u64 = 0;

/// 10ms Interval
const TIMER_INTERVAL: Duration = Duration::from_millis(10);
const TICKS_PER_SECOND: u64 =
    (Duration::from_secs(1).as_nanos() / TIMER_INTERVAL.as_nanos()) as u64;

#[ctor]
static TIMER_INTERVAL_CNT: u64 = compute_timer_counter_value(TIMER_INTERVAL);

fn compute_timer_counter_value(duration: core::time::Duration) -> u64 {
    let freq = unsafe { core::ptr::read_volatile(&TIMER_FREQ) } as f64;
    (freq * duration.as_secs_f64()) as u64
}

fn set_timer_interval_count() {
    let timer = *TIMER_INTERVAL_CNT;
    // Set timer interval
    CNTP_TVAL_EL0.set(timer);
}

const TIMER_IRQ_PENDING_BIT_NUM: IRQNum = 0;

#[derive(Default)]
struct TimerInterruptHandler {
    ticks: Mutex<u64>,
}

impl IRQHandler for TimerInterruptHandler {
    fn get_irq_pending_bit_num(&self) -> IRQNum {
        TIMER_IRQ_PENDING_BIT_NUM
    }

    fn handle(&self, _ec: &mut ExceptionContext) {
        let tick_count;
        {
            let mut ticks = self.ticks.lock();
            tick_count = *ticks;
            *ticks += 1;
        }
        if tick_count % TICKS_PER_SECOND == 0 {
            println!(
                "Time Elapsed Since Boot = {} s",
                tick_count / TICKS_PER_SECOND
            );
        }
        set_timer_interval_count();
    }
}

#[ctor]
static IRQ_HANDLER: TimerInterruptHandler = TimerInterruptHandler::default();

/// .
///
/// # Safety
///
/// Init Timer module
pub unsafe fn enable() {
    set_timer_interval_count();

    // Enable timer and timer interrupt
    CNTP_CTL_EL0.write(CNTP_CTL_EL0::ENABLE::SET + CNTP_CTL_EL0::IMASK::CLEAR);

    CNTP_EL0.write_reg(1u64 << 1);
    register_interrupt_handler(&*IRQ_HANDLER);
}
