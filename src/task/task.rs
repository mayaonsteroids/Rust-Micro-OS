//! Core Task representation (Task Control Block), unique identification, and metadata.

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::paging::PhysFrame;
use crate::task::context::TaskContext;
use crate::task::state::{StateTransitionError, TaskState};

/// Global atomic counter for unique sequential task IDs.
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// A strongly typed, unique identifier for each kernel task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

/// Sentinel value indicating that the 64-bit TaskId space is permanently exhausted.
pub const TASK_ID_EXHAUSTED_SENTINEL: u64 = u64::MAX;

/// Maximum allowable allocated TaskId (valid IDs: 1 ..= u64::MAX - 1).
pub const MAX_ALLOCATABLE_TASK_ID: u64 = u64::MAX - 1;

/// Error returned when TaskId counter reaches exhaustion sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskIdExhausted;

impl TaskId {
    /// Allocates a new unique Task ID atomically using a fail-closed compare-and-swap loop.
    ///
    /// Multi-core (SMP) safe: employs `Ordering::AcqRel` on success and `Ordering::Acquire`
    /// on failure to prevent any race condition, torn read, or ABA hazard across cores.
    pub fn allocate() -> Result<Self, TaskIdExhausted> {
        let mut current = NEXT_TASK_ID.load(Ordering::Acquire);
        loop {
            if current >= MAX_ALLOCATABLE_TASK_ID {
                return Err(TaskIdExhausted);
            }
            let next = current + 1;
            match NEXT_TASK_ID.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Self(current)),
                Err(actual) => current = actual,
            }
        }
    }

    /// Generates a new unique Task ID atomically.
    pub fn new() -> Self {
        Self::allocate().unwrap_or(Self(MAX_ALLOCATABLE_TASK_ID))
    }

    /// Returns the raw integer value of the TaskId.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T{}", self.0)
    }
}

/// Task execution priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

impl Priority {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "LOW",
            Priority::Normal => "NORMAL",
            Priority::High => "HIGH",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Function pointer type for bare-metal task entry points.
pub type TaskFn = fn();

/// Core Task Control Block (TCB) representation.
///
/// Encapsulates all identity, scheduling, and execution state metadata
/// for an independent unit of execution in the Micro-OS.
#[derive(Debug, Clone, Copy)]
pub struct Task {
    /// Unique sequential task identifier.
    pub id: TaskId,
    /// Human-readable task name for diagnostics and logging.
    pub name: &'static str,
    /// Current life-cycle state.
    pub state: TaskState,
    /// Scheduling priority.
    pub priority: Priority,
    /// Entry-point function pointer.
    pub entry_point: Option<TaskFn>,
    /// CPU register context frame.
    pub context: TaskContext,
    /// Total CPU runtime ticks consumed.
    pub ticks: u64,
    /// Optional Level 4 Page Table (CR3) for user-mode address space isolation.
    pub cr3: Option<PhysFrame>,
}

impl Task {
    /// Creates a new task in the `READY` state with default `NORMAL` priority.
    pub fn new(name: &'static str, entry_point: Option<TaskFn>) -> Self {
        Self::new_with_priority(name, Priority::Normal, entry_point)
    }

    /// Creates a new task with a specified priority.
    pub fn new_with_priority(
        name: &'static str,
        priority: Priority,
        entry_point: Option<TaskFn>,
    ) -> Self {
        let mut context = TaskContext::new();
        if let Some(entry) = entry_point {
            context.rip = entry as usize as u64;
        }

        Self {
            id: TaskId::new(),
            name,
            state: TaskState::Ready,
            priority,
            entry_point,
            context,
            ticks: 0,
            cr3: None,
        }
    }

    /// Sets or clears the task's CR3 address space.
    pub fn set_cr3(&mut self, cr3: Option<PhysFrame>) {
        self.cr3 = cr3;
    }

    /// Transitions the task to a target state, enforcing state-machine validity.
    pub fn transition_to(&mut self, target: TaskState) -> Result<(), StateTransitionError> {
        if self.state.can_transition_to(target) {
            self.state = target;
            Ok(())
        } else {
            Err(StateTransitionError {
                from: self.state,
                to: target,
            })
        }
    }

    /// Terminates the task, transitioning it to `TERMINATED`.
    pub fn terminate(&mut self) -> Result<(), StateTransitionError> {
        self.transition_to(TaskState::Terminated)
    }

    /// Returns true if the task is eligible to run (`READY` state).
    pub const fn is_ready(&self) -> bool {
        matches!(self.state, TaskState::Ready)
    }

    /// Returns true if the task is currently active on the CPU (`RUNNING` state).
    pub const fn is_running(&self) -> bool {
        matches!(self.state, TaskState::Running)
    }

    /// Returns true if the task is blocked waiting for an event (`BLOCKED` state).
    pub const fn is_blocked(&self) -> bool {
        matches!(self.state, TaskState::Blocked)
    }

    /// Returns true if the task has terminated.
    pub const fn is_terminated(&self) -> bool {
        matches!(self.state, TaskState::Terminated)
    }

    /// Increments the task's runtime execution tick counter.
    pub fn increment_ticks(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Task({}, \"{}\", state={}, prio={}, ticks={})",
            self.id, self.name, self.state, self.priority, self.ticks
        )
    }
}
