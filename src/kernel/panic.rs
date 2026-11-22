//! Panic handling subsystem.

use core::panic::PanicInfo;

/// Halts the CPU loop upon unrecoverable kernel panic.
pub fn handle_panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
