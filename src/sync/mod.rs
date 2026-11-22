//! Kernel synchronization primitives and mutual exclusion mechanisms.
//!
//! Provides bare-metal synchronization mechanisms conforming to Milestone 10 requirements:
//! - `SpinLock<T>`: Atomic test-and-set spinlock with RAII guard and contention metrics.
//! - `IrqSpinLock<T>`: Interrupt-safe spinlock preventing ISR-thread deadlocks.
//! - `TicketLock<T>`: Fair FIFO spinlock preventing starvation under heavy contention.
//!
//! Also provides the in-kernel unit test suite and live race condition demonstration (`tests`).

pub mod spinlock;
pub mod irq_spinlock;
pub mod ticket_lock;
pub mod tests;

pub use spinlock::{SpinLock, SpinLockGuard};
pub use irq_spinlock::{IrqSpinLock, IrqSpinLockGuard};
pub use ticket_lock::{TicketLock, TicketLockGuard};
