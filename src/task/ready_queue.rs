//! FIFO Ready Queue implementation backed by a fixed-capacity circular buffer.
//!
//! Stores `TaskId` entries for tasks that are currently in the `READY` state,
//! serving as the core queue for the Round Robin scheduler.
//! Supports two-phase capacity reservations with linear move-only tokens.

use core::fmt;
use spin::Mutex;
use crate::task::manager::{TaskManager, TASK_MANAGER};
use crate::task::state::TaskState;
use crate::task::task::TaskId;

/// Fixed maximum capacity of the Ready Queue, matching the task table limit.
pub const QUEUE_CAPACITY: usize = 32;

/// Maximum number of concurrent active capacity reservations.
pub const MAX_RESERVATIONS: usize = 4;

/// Timeout for active reservations in timer ticks (100 ticks = 1.0 second at 100 Hz).
pub const RESERVATION_TIMEOUT_TICKS: u64 = 100;

/// Error types returned during Ready Queue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// The queue has reached maximum capacity (taking active reservations into account).
    QueueFull,
    /// The task is already present in the queue (duplicates are forbidden).
    DuplicateTask(TaskId),
    /// The task is not in the `READY` state and cannot enter the queue.
    TaskNotReady(TaskId, TaskState),
    /// The task ID was not found in the task manager.
    TaskNotFound(TaskId),
    /// Reservation sequence counter exhausted.
    SequenceExhausted,
    /// No reservation slots available.
    NoReservationSlots,
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::QueueFull => write!(f, "Ready Queue is full (capacity: {})", QUEUE_CAPACITY),
            QueueError::DuplicateTask(id) => write!(f, "Task {} is already in the Ready Queue", id),
            QueueError::TaskNotReady(id, state) => {
                write!(f, "Task {} is in state [{}] (must be READY to enter queue)", id, state)
            }
            QueueError::TaskNotFound(id) => write!(f, "Task {} not found in TaskManager", id),
            QueueError::SequenceExhausted => write!(f, "Reservation sequence counter exhausted"),
            QueueError::NoReservationSlots => write!(f, "All reservation tracking slots are in use"),
        }
    }
}

/// Errors occurring during reservation commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitError {
    ForeignQueueToken,
    TokenAlreadyConsumed,
    InvalidReservationToken,
    ReservationExpired,
    DuplicateTask(TaskId),
    QueueFull,
}

/// Errors occurring during reservation cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelError {
    ForeignQueueToken,
    InvalidReservationToken,
    TokenAlreadyConsumed,
}

/// Identification for a queue reservation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReservationId {
    pub queue_id: u64,
    pub sequence: u64,
}

/// In-queue tracking entry for an active reservation.
#[derive(Copy, Clone, Debug)]
pub struct ReservationEntry {
    pub id: ReservationId,
    pub created_tick: u64,
    pub active: bool,
}

/// Strongly typed, linear, move-only reservation token.
///
/// Intentionally does NOT implement Copy, Clone, or Drop.
/// Fields are strictly private to prevent external fabrication.
#[derive(Debug)]
pub struct QueueReservation {
    queue_id: u64,
    sequence: u64,
}

impl QueueReservation {
    pub const fn queue_id(&self) -> u64 {
        self.queue_id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// Circular FIFO Ready Queue storing TaskId entries without dynamic allocation.
#[derive(Debug, Clone, Copy)]
pub struct ReadyQueue {
    buffer: [Option<TaskId>; QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
    queue_id: u64,
    reserved_count: usize,
    reservations: [Option<ReservationEntry>; MAX_RESERVATIONS],
    next_sequence: u64,
}

impl ReadyQueue {
    /// Creates a new, empty ReadyQueue.
    pub const fn new() -> Self {
        Self {
            buffer: [None; QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
            queue_id: 1,
            reserved_count: 0,
            reservations: [None; MAX_RESERVATIONS],
            next_sequence: 1,
        }
    }

    /// Reserves a slot in the Ready Queue, returning a linear move-only token.
    pub fn reserve(&mut self, current_tick: u64) -> Result<QueueReservation, QueueError> {
        // 1. Sweep expired reservations
        self.sweep_expired_reservations(current_tick);

        // 2. Check total capacity invariant: count + reserved_count < QUEUE_CAPACITY
        if self.count + self.reserved_count >= QUEUE_CAPACITY {
            return Err(QueueError::QueueFull);
        }

        // 3. Find an available reservation slot
        let mut target_idx = None;
        for (i, slot) in self.reservations.iter().enumerate() {
            if slot.is_none() || !slot.unwrap().active {
                target_idx = Some(i);
                break;
            }
        }

        let idx = target_idx.ok_or(QueueError::NoReservationSlots)?;

        if self.next_sequence >= u64::MAX - 1 {
            return Err(QueueError::SequenceExhausted);
        }

        let seq = self.next_sequence;
        self.next_sequence += 1;

        let entry = ReservationEntry {
            id: ReservationId {
                queue_id: self.queue_id,
                sequence: seq,
            },
            created_tick: current_tick,
            active: true,
        };

        self.reservations[idx] = Some(entry);
        self.reserved_count += 1;

        Ok(QueueReservation {
            queue_id: self.queue_id,
            sequence: seq,
        })
    }

    /// Commits an existing reservation by enqueuing the task.
    ///
    /// Consumes the token by value:
    /// - On success: consumes token, enqueues task, decrements `reserved_count`, increments `count`.
    /// - On failure: leaves queue state unmodified and returns the unconsumed token.
    pub fn commit_reservation(
        &mut self,
        res: QueueReservation,
        task: TaskId,
        current_tick: u64,
    ) -> Result<(), (CommitError, QueueReservation)> {
        if res.queue_id != self.queue_id {
            return Err((CommitError::ForeignQueueToken, res));
        }

        let mut found = None;
        for (i, slot) in self.reservations.iter().enumerate() {
            if let Some(entry) = slot {
                if entry.id.sequence == res.sequence {
                    found = Some((i, entry.active, entry.created_tick));
                    break;
                }
            }
        }

        let (idx, active, created_tick) = match found {
            Some(f) => f,
            None => return Err((CommitError::InvalidReservationToken, res)),
        };

        if !active {
            return Err((CommitError::TokenAlreadyConsumed, res));
        }

        // Check if reservation expired
        if current_tick.saturating_sub(created_tick) >= RESERVATION_TIMEOUT_TICKS {
            self.reservations[idx].as_mut().unwrap().active = false;
            self.reserved_count = self.reserved_count.saturating_sub(1);
            return Err((CommitError::ReservationExpired, res));
        }

        if self.contains(task) {
            return Err((CommitError::DuplicateTask(task), res));
        }

        if self.count >= QUEUE_CAPACITY {
            return Err((CommitError::QueueFull, res));
        }

        // Commit task to ring buffer
        self.buffer[self.tail] = Some(task);
        self.tail = (self.tail + 1) % QUEUE_CAPACITY;
        self.count += 1;

        // Deactivate reservation slot and decrement reserved_count
        self.reservations[idx].as_mut().unwrap().active = false;
        self.reserved_count = self.reserved_count.saturating_sub(1);

        Ok(())
    }

    /// Explicitly cancels an active reservation, releasing the reserved capacity.
    pub fn cancel_reservation(
        &mut self,
        res: QueueReservation,
    ) -> Result<(), (CancelError, QueueReservation)> {
        if res.queue_id != self.queue_id {
            return Err((CancelError::ForeignQueueToken, res));
        }

        let mut found = None;
        for (i, slot) in self.reservations.iter().enumerate() {
            if let Some(entry) = slot {
                if entry.id.sequence == res.sequence {
                    found = Some((i, entry.active));
                    break;
                }
            }
        }

        let (idx, active) = match found {
            Some(f) => f,
            None => return Err((CancelError::InvalidReservationToken, res)),
        };

        if !active {
            return Err((CancelError::TokenAlreadyConsumed, res));
        }

        self.reservations[idx].as_mut().unwrap().active = false;
        self.reserved_count = self.reserved_count.saturating_sub(1);

        Ok(())
    }

    /// Sweeps active reservations and deactivates any that exceeded the timeout.
    pub fn sweep_expired_reservations(&mut self, current_tick: u64) -> usize {
        let mut expired_count = 0;
        for slot in self.reservations.iter_mut() {
            if let Some(entry) = slot {
                if entry.active && current_tick.saturating_sub(entry.created_tick) >= RESERVATION_TIMEOUT_TICKS {
                    entry.active = false;
                    self.reserved_count = self.reserved_count.saturating_sub(1);
                    expired_count += 1;
                }
            }
        }
        expired_count
    }

    /// Adds a task to the queue after validating its existence and READY state against TaskManager.
    pub fn enqueue(&mut self, id: TaskId, tm: &TaskManager) -> Result<(), QueueError> {
        let task = tm.get_task(id).ok_or(QueueError::TaskNotFound(id))?;
        if task.state != TaskState::Ready {
            return Err(QueueError::TaskNotReady(id, task.state));
        }
        self.enqueue_raw(id)
    }

    /// Internal low-level enqueue for raw data-structure operations.
    pub(crate) fn enqueue_raw(&mut self, id: TaskId) -> Result<(), QueueError> {
        if self.count + self.reserved_count >= QUEUE_CAPACITY {
            return Err(QueueError::QueueFull);
        }
        if self.contains(id) {
            return Err(QueueError::DuplicateTask(id));
        }

        self.buffer[self.tail] = Some(id);
        self.tail = (self.tail + 1) % QUEUE_CAPACITY;
        self.count += 1;
        Ok(())
    }

    /// Removes and returns the task at the front of the queue (FIFO order).
    pub fn dequeue(&mut self) -> Option<TaskId> {
        if self.is_empty() {
            return None;
        }

        let id = self.buffer[self.head].take();
        self.head = (self.head + 1) % QUEUE_CAPACITY;
        self.count -= 1;
        id
    }

    /// Returns the task at the front of the queue without removing it.
    pub fn peek(&self) -> Option<TaskId> {
        if self.is_empty() {
            None
        } else {
            self.buffer[self.head]
        }
    }

    /// Returns true if the queue contains no tasks.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns true if the queue has reached its maximum capacity.
    pub const fn is_full(&self) -> bool {
        self.count + self.reserved_count >= QUEUE_CAPACITY
    }

    /// Returns the number of tasks currently in the queue.
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Returns the number of active reservations.
    pub const fn reserved_len(&self) -> usize {
        self.reserved_count
    }

    /// Checks whether a task is already present in the queue.
    pub fn contains(&self, id: TaskId) -> bool {
        for i in 0..self.count {
            let idx = (self.head + i) % QUEUE_CAPACITY;
            if self.buffer[idx] == Some(id) {
                return true;
            }
        }
        false
    }

    /// Removes a specific task from anywhere in the queue, preserving FIFO order.
    pub fn remove(&mut self, id: TaskId) -> bool {
        let mut found_offset = None;
        for i in 0..self.count {
            let idx = (self.head + i) % QUEUE_CAPACITY;
            if self.buffer[idx] == Some(id) {
                found_offset = Some(i);
                break;
            }
        }

        if let Some(offset) = found_offset {
            for j in offset..(self.count - 1) {
                let curr = (self.head + j) % QUEUE_CAPACITY;
                let next = (self.head + j + 1) % QUEUE_CAPACITY;
                self.buffer[curr] = self.buffer[next];
            }
            let last_idx = (self.head + self.count - 1) % QUEUE_CAPACITY;
            self.buffer[last_idx] = None;
            self.tail = last_idx;
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Clears all tasks and reservations safely.
    pub fn clear(&mut self) {
        for slot in self.buffer.iter_mut() {
            *slot = None;
        }
        for res in self.reservations.iter_mut() {
            *res = None;
        }
        self.head = 0;
        self.tail = 0;
        self.count = 0;
        self.reserved_count = 0;
    }

    /// Returns an iterator over the queued tasks in FIFO order.
    pub fn iter(&self) -> impl Iterator<Item = TaskId> + '_ {
        (0..self.count).map(move |i| {
            let idx = (self.head + i) % QUEUE_CAPACITY;
            self.buffer[idx].expect("Valid queue slot must contain TaskId")
        })
    }
}

/// Global synchronized ReadyQueue instance protected by a spinlock.
pub static READY_QUEUE: Mutex<ReadyQueue> = Mutex::new(ReadyQueue::new());

/// High-level helper: validates task against TaskManager, then enqueues.
pub fn enqueue_task(id: TaskId) -> Result<(), QueueError> {
    let tm = TASK_MANAGER.lock();
    let mut rq = READY_QUEUE.lock();
    rq.enqueue(id, &tm)
}

/// Public alias for enqueue_task.
pub fn enqueue(id: TaskId) -> Result<(), QueueError> {
    enqueue_task(id)
}

/// High-level helper: dequeues the front task.
pub fn dequeue_task() -> Option<TaskId> {
    READY_QUEUE.lock().dequeue()
}

/// High-level helper: peeks at the front task.
pub fn peek_task() -> Option<TaskId> {
    READY_QUEUE.lock().peek()
}

/// High-level helper: removes a task from the ReadyQueue.
pub fn remove_task(id: TaskId) -> bool {
    READY_QUEUE.lock().remove(id)
}
