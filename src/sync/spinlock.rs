//! Custom Kernel SpinLock implementation with RAII Guard and contention metrics.
//!
//! Provides mutual exclusion for bare-metal multi-tasking environments.
//! Encapsulates protected data inside `UnsafeCell<T>` and provides safe access
//! through the `SpinLockGuard<T>` RAII type.
//!
//! Conforms to Milestone 10 requirements and Track C Concurrency Engineering specifications.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A hardware-level mutual exclusion spinlock protecting data of type `T`.
///
/// Ensures mutual exclusion between concurrent tasks on single or multi-core systems.
/// Uses atomic compare-and-swap operations with acquire-release memory semantics.
pub struct SpinLock<T: ?Sized> {
    /// Atomic flag indicating whether the lock is currently held (true = locked, false = free).
    locked: AtomicBool,
    /// Total number of successful lock acquisitions.
    acquire_count: AtomicU64,
    /// Total number of times a task contended for this lock (spun at least once).
    contention_count: AtomicU64,
    /// The inner data protected by this spinlock.
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a new unlocked `SpinLock` protecting the given data.
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            acquire_count: AtomicU64::new(0),
            contention_count: AtomicU64::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the spinlock and returns the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> SpinLock<T> {
    /// Acquires the spinlock, spinning until the lock becomes available.
    ///
    /// Returns an RAII `SpinLockGuard` that grants exclusive mutable access to the protected data.
    /// The lock is automatically released when the guard is dropped.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // Fast path: try to acquire immediately without spinning
        if self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // Contention detected: record metric and enter spin loop
            self.contention_count.fetch_add(1, Ordering::Relaxed);
            while self.locked.swap(true, Ordering::Acquire) {
                // Issue x86 PAUSE instruction to optimize pipeline and save power
                core::hint::spin_loop();
            }
        }

        self.acquire_count.fetch_add(1, Ordering::Relaxed);
        SpinLockGuard { lock: self }
    }

    /// Attempts to acquire the spinlock without blocking/spinning.
    ///
    /// If the lock is currently free, it is acquired and `Some(SpinLockGuard)` is returned.
    /// If the lock is currently held, `None` is returned immediately.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            self.acquire_count.fetch_add(1, Ordering::Relaxed);
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    /// Returns `true` if the spinlock is currently held by any task.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    /// Returns a snapshot of lock performance metrics: `(acquire_count, contention_count)`.
    pub fn metrics(&self) -> (u64, u64) {
        (
            self.acquire_count.load(Ordering::Relaxed),
            self.contention_count.load(Ordering::Relaxed),
        )
    }

    /// Resets the contention and acquisition counters to zero.
    pub fn reset_metrics(&self) {
        self.acquire_count.store(0, Ordering::Relaxed);
        self.contention_count.store(0, Ordering::Relaxed);
    }

    /// Returns a raw mutable pointer to the underlying data.
    ///
    /// # Safety
    /// Caller must ensure that mutual exclusion is maintained manually.
    pub unsafe fn get_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

/// An RAII guard that grants exclusive mutable access to data protected by a `SpinLock`.
///
/// When dropped, the spinlock is automatically unlocked.
pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a SpinLock<T>,
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // Safe: existence of guard proves exclusive access is held
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // Safe: existence of guard proves exclusive access is held
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        // Release lock using Release memory ordering so preceding writes are visible
        self.lock.locked.store(false, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SpinLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("SpinLock").field("data", &&*guard).finish(),
            None => f.debug_struct("SpinLock").field("data", &"<locked>").finish(),
        }
    }
}
