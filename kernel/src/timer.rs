use core::time::Duration;

use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_TVAL_EL0};
use spin::Mutex;
use tock_registers::interfaces::{Readable, Writeable};

use crate::{exception::ExceptionContext, println};

/// Will be initialized by ASM (boot.s)
#[no_mangle]
static TIMER_FREQ: u64 = 0;

/// 10ms Interval
const TIMER_INTERVAL: Duration = Duration::from_millis(10);
const TICKS_PER_SECOND: u64 =
    (Duration::from_secs(1).as_nanos() / TIMER_INTERVAL.as_nanos()) as u64;
lazy_static! {
    static ref TIMER_INTERVAL_CNT: u64 = compute_timer_counter_value(TIMER_INTERVAL);
}

fn compute_timer_counter_value(duration: core::time::Duration) -> u64 {
    let freq = unsafe { core::ptr::read_volatile(&TIMER_FREQ) } as f64;
    (freq * duration.as_secs_f64()) as u64
}

fn set_timer_interval_count() {
    let timer = *TIMER_INTERVAL_CNT;
    // Set timer interval
    CNTP_TVAL_EL0.set(timer);
}

/// .
///
/// # Safety
///
/// Init Timer module
pub unsafe fn init_timer() {
    set_timer_interval_count();

    // Enable timer and timer interrupt
    CNTP_CTL_EL0.write(CNTP_CTL_EL0::ENABLE::SET + CNTP_CTL_EL0::IMASK::CLEAR);

    let cntp_el0 = 0x40000040 as *mut u64;
    core::ptr::write_volatile(cntp_el0, 1 << 1);
}

pub(crate) fn is_timer_irq() -> bool {
    let cntp_status_el0 = 0x40000060 as *mut u64;
    unsafe {
        core::ptr::read_volatile(cntp_status_el0) & (1 << 1) != 0
            && CNTP_CTL_EL0.is_set(CNTP_CTL_EL0::ISTATUS)
    }
}

static TICKS: Mutex<u64> = Mutex::new(0);

pub(crate) fn handle_timer_irq(_ec: &mut ExceptionContext) {
    let tick_count;
    {
        let mut ticks = TICKS.lock();
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
