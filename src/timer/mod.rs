//! Hardware timer interface (PIT) and interrupt tick tracking.

pub mod pit;

use core::sync::atomic::{AtomicU64, Ordering};

/// Monotonically increasing tick counter updated by timer interrupt handler.
pub static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Fetch the current elapsed timer ticks since boot.
pub fn get_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// Reset the tick counter (useful for benchmark timing).
pub fn reset_ticks() {
    TIMER_TICKS.store(0, Ordering::SeqCst);
}

/// Increments the tick counter by one.
#[inline(always)]
pub fn increment_ticks() -> u64 {
    TIMER_TICKS.fetch_add(1, Ordering::SeqCst) + 1
}
