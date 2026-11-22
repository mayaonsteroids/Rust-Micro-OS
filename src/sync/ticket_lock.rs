//! Fair Ticket Spinlock implementation.
//!
//! Provides FIFO-ordered synchronization preventing task starvation under heavy contention.
//! Part of the CA-6 Track C Concurrency Engineering extension.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU64, Ordering};

/// A FIFO-ordered Ticket Spinlock protecting inner data of type `T`.
///
/// Ensures fair scheduling of lock acquisitions: tasks receive an atomic ticket number
/// upon arrival and are served in exact numerical order.
pub struct TicketLock<T: ?Sized> {
    /// Next ticket number to be dispensed to an arriving task.
    next_ticket: AtomicU64,
    /// Ticket number currently permitted to access the critical section.
    now_serving: AtomicU64,
    /// Total number of contentions observed (tasks forced to wait).
    contention_count: AtomicU64,
    /// Inner data protected by the lock.
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for TicketLock<T> {}
unsafe impl<T: ?Sized + Send> Send for TicketLock<T> {}

impl<T> TicketLock<T> {
    /// Creates a new unlocked `TicketLock` protecting the given data.
    pub const fn new(data: T) -> Self {
        Self {
            next_ticket: AtomicU64::new(0),
            now_serving: AtomicU64::new(0),
            contention_count: AtomicU64::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the lock and returns the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> TicketLock<T> {
    /// Acquires the ticket lock in strict FIFO arrival order.
    pub fn lock(&self) -> TicketLockGuard<'_, T> {
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);

        if self.now_serving.load(Ordering::Acquire) != my_ticket {
            self.contention_count.fetch_add(1, Ordering::Relaxed);
            while self.now_serving.load(Ordering::Acquire) != my_ticket {
                core::hint::spin_loop();
            }
        }

        TicketLockGuard {
            lock: self,
            _ticket: my_ticket,
        }
    }

    /// Attempts to acquire the ticket lock without blocking.
    ///
    /// Succeeds only if no other tasks are currently waiting or holding the lock.
    pub fn try_lock(&self) -> Option<TicketLockGuard<'_, T>> {
        let current = self.now_serving.load(Ordering::Acquire);
        if self.next_ticket.compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(TicketLockGuard {
                lock: self,
                _ticket: current,
            })
        } else {
            None
        }
    }

    /// Returns `true` if the lock is currently held by any task.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.now_serving.load(Ordering::Relaxed) != self.next_ticket.load(Ordering::Relaxed)
    }

    /// Returns the number of tasks currently waiting in line for this lock.
    pub fn queue_depth(&self) -> u64 {
        let next = self.next_ticket.load(Ordering::Relaxed);
        let serving = self.now_serving.load(Ordering::Relaxed);
        next.saturating_sub(serving)
    }

    /// Returns the total contention count recorded.
    pub fn contention_count(&self) -> u64 {
        self.contention_count.load(Ordering::Relaxed)
    }
}

/// RAII guard for a `TicketLock`.
///
/// Increments `now_serving` on drop to signal the next queued task.
pub struct TicketLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a TicketLock<T>,
    _ticket: u64,
}

impl<'a, T: ?Sized> Deref for TicketLockGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for TicketLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for TicketLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        // Increment serving counter to wake up the next task in line
        self.lock.now_serving.fetch_add(1, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for TicketLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("TicketLock").field("data", &&*guard).finish(),
            None => f.debug_struct("TicketLock").field("data", &"<locked>").finish(),
        }
    }
}
