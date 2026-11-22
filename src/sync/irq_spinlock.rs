//! Interrupt-safe SpinLock implementation.
//!
//! Prevents deadlocks between hardware interrupt handlers (such as the PIT timer ISR)
//! and kernel tasks by disabling interrupts on the local CPU while the spinlock is held.

use core::fmt;
use core::ops::{Deref, DerefMut};
use crate::sync::spinlock::{SpinLock, SpinLockGuard};

/// An interrupt-safe spinlock wrapper protecting data of type `T`.
///
/// Automatically disables hardware interrupts (`cli`) before acquiring the lock,
/// and restores the previous interrupt enable state (`sti` if previously enabled)
/// when the lock is dropped.
pub struct IrqSpinLock<T: ?Sized> {
    inner: SpinLock<T>,
}

unsafe impl<T: ?Sized + Send> Sync for IrqSpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for IrqSpinLock<T> {}

impl<T> IrqSpinLock<T> {
    /// Creates a new interrupt-safe spinlock protecting the given data.
    pub const fn new(data: T) -> Self {
        Self {
            inner: SpinLock::new(data),
        }
    }

    /// Consumes the lock and returns the underlying data.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T: ?Sized> IrqSpinLock<T> {
    /// Disables interrupts and acquires the spinlock.
    ///
    /// The previous interrupt state is recorded and restored upon dropping the guard.
    ///
    /// # Deadlock Prevention on Single-Core Systems
    /// If the lock is already held by another task, interrupts are temporarily re-enabled
    /// while waiting so that timer preemption can schedule the lock holder to complete
    /// its critical section and release the lock.
    pub fn lock(&self) -> IrqSpinLockGuard<'_, T> {
        let was_enabled = x86_64::instructions::interrupts::are_enabled();
        loop {
            x86_64::instructions::interrupts::disable();
            if let Some(guard) = self.inner.try_lock() {
                return IrqSpinLockGuard {
                    guard: Some(guard),
                    interrupts_were_enabled: was_enabled,
                };
            }
            // Restore interrupts while waiting so timer preemption can schedule the lock holder
            if was_enabled {
                x86_64::instructions::interrupts::enable();
            }
            core::hint::spin_loop();
        }
    }

    /// Attempts to acquire the spinlock without blocking.
    ///
    /// If the lock cannot be acquired immediately, interrupts are restored to their previous
    /// state and `None` is returned.
    pub fn try_lock(&self) -> Option<IrqSpinLockGuard<'_, T>> {
        let was_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();

        match self.inner.try_lock() {
            Some(guard) => Some(IrqSpinLockGuard {
                guard: Some(guard),
                interrupts_were_enabled: was_enabled,
            }),
            None => {
                // Restore interrupts if they were enabled prior to attempt
                if was_enabled {
                    x86_64::instructions::interrupts::enable();
                }
                None
            }
        }
    }

    /// Returns `true` if the lock is currently held.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    /// Returns lock performance metrics: `(acquire_count, contention_count)`.
    pub fn metrics(&self) -> (u64, u64) {
        self.inner.metrics()
    }

    /// Resets contention and acquisition metrics.
    pub fn reset_metrics(&self) {
        self.inner.reset_metrics();
    }
}

/// RAII guard for an `IrqSpinLock`.
///
/// Releases the inner spinlock and restores the CPU's interrupt state when dropped.
pub struct IrqSpinLockGuard<'a, T: ?Sized + 'a> {
    guard: Option<SpinLockGuard<'a, T>>,
    interrupts_were_enabled: bool,
}

impl<'a, T: ?Sized> Deref for IrqSpinLockGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.guard.as_ref().unwrap()
    }
}

impl<'a, T: ?Sized> DerefMut for IrqSpinLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_mut().unwrap()
    }
}

impl<'a, T: ?Sized> Drop for IrqSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        // Drop inner spinlock guard first (restoring locked = false)
        drop(self.guard.take());

        // Restore interrupt state
        if self.interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for IrqSpinLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("IrqSpinLock").field("data", &&*guard).finish(),
            None => f.debug_struct("IrqSpinLock").field("data", &"<locked>").finish(),
        }
    }
}
