//! Sleep Queue and timer-driven task wakeup subsystem.
//!
//! Part of Milestone 12: Process/Thread Research Specialization (Track G / Paper R6).

use spin::Mutex;
use crate::task::task::TaskId;
use crate::task::state::TaskState;
use crate::task::manager::TASK_MANAGER;
use crate::task::ready_queue::READY_QUEUE;
use crate::scheduler::round_robin::{SCHEDULER, sys_yield};

/// Maximum capacity of the sleep queue.
pub const MAX_SLEEPING_TASKS: usize = 32;

/// A record representing a task sleeping until a target timer tick.
///
/// Size: 16 bytes (`task_id`: 8 B, `wake_tick`: 8 B), alignment 8 bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SleepEntry {
    pub task_id: TaskId,
    pub wake_tick: u64,
}

impl SleepEntry {
    pub const fn new(task_id: TaskId, wake_tick: u64) -> Self {
        Self { task_id, wake_tick }
    }
}

/// Static priority-less delta/tick sleep queue.
///
/// Size: `entries: [Option<SleepEntry>; 32]` (768 B) + `count: usize` (8 B) = 776 bytes, align 8.
pub struct SleepQueue {
    pub entries: [Option<SleepEntry>; MAX_SLEEPING_TASKS],
    pub count: usize,
}

impl SleepQueue {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_SLEEPING_TASKS],
            count: 0,
        }
    }

    /// Enqueues a task to sleep until `wake_tick`.
    pub fn sleep(&mut self, task_id: TaskId, wake_tick: u64) -> Result<(), SleepError> {
        if self.count >= MAX_SLEEPING_TASKS {
            return Err(SleepError::QueueFull);
        }

        // Check if task is already sleeping
        for slot in self.entries.iter() {
            if let Some(entry) = slot {
                if entry.task_id == task_id {
                    return Err(SleepError::AlreadySleeping);
                }
            }
        }

        // Find an empty slot
        for slot in self.entries.iter_mut() {
            if slot.is_none() {
                *slot = Some(SleepEntry::new(task_id, wake_tick));
                self.count += 1;
                return Ok(());
            }
        }

        Err(SleepError::QueueFull)
    }

    /// Removes a task from the sleep queue (e.g. upon early abortion or exit).
    pub fn remove(&mut self, task_id: TaskId) -> bool {
        for slot in self.entries.iter_mut() {
            if let Some(entry) = slot {
                if entry.task_id == task_id {
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                    return true;
                }
            }
        }
        false
    }

    /// Checks for tasks whose wake tick has arrived and removes them from the sleep queue.
    /// Returns a fixed-size array of woken task IDs.
    pub fn poll_wakeups(&mut self, current_tick: u64) -> [Option<TaskId>; MAX_SLEEPING_TASKS] {
        let mut woken = [None; MAX_SLEEPING_TASKS];
        let mut woken_idx = 0;

        for slot in self.entries.iter_mut() {
            if let Some(entry) = slot {
                if entry.wake_tick <= current_tick {
                    if woken_idx < MAX_SLEEPING_TASKS {
                        woken[woken_idx] = Some(entry.task_id);
                        woken_idx += 1;
                    }
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                }
            }
        }

        woken
    }

    pub fn is_sleeping(&self, task_id: TaskId) -> bool {
        self.entries.iter().any(|slot| {
            if let Some(entry) = slot {
                entry.task_id == task_id
            } else {
                false
            }
        })
    }
}

/// Global SleepQueue protected by a SpinMutex.
pub static SLEEP_QUEUE: Mutex<SleepQueue> = Mutex::new(SleepQueue::new());

/// Error types for Sleep operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepError {
    QueueFull,
    AlreadySleeping,
    NoActiveTask,
}

/// Called during timer tick processing to wake up sleeping tasks.
pub fn check_wakeups(current_tick: u64) -> usize {
    let woken = SLEEP_QUEUE.lock().poll_wakeups(current_tick);
    let mut wake_count = 0;

    for task_opt in woken.iter() {
        if let Some(tid) = task_opt {
            // Transition task from Blocked to Ready
            let mut tm = TASK_MANAGER.lock();
            let mut rq = READY_QUEUE.lock();
            if let Ok(()) = tm.transition_task_sync(*tid, TaskState::Ready, &mut rq) {
                wake_count += 1;
            }
        }
    }

    wake_count
}

/// Cooperative sys_sleep: puts current running task to sleep for `duration_ticks`.
pub fn sys_sleep(duration_ticks: u64, current_tick: u64) -> Result<(), SleepError> {
    let curr_id = match SCHEDULER.lock().current {
        Some(id) => id,
        None => return Err(SleepError::NoActiveTask),
    };

    let wake_tick = current_tick.saturating_add(duration_ticks);

    // 1. Enqueue to SLEEP_QUEUE
    SLEEP_QUEUE.lock().sleep(curr_id, wake_tick)?;

    // 2. Transition task to BLOCKED
    {
        let mut tm = TASK_MANAGER.lock();
        let mut rq = READY_QUEUE.lock();
        let _ = tm.transition_task_sync(curr_id, TaskState::Blocked, &mut rq);
    }

    // 3. Yield CPU to next ready task
    sys_yield();

    Ok(())
}
