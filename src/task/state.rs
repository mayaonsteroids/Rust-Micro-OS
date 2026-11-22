//! Task execution states, transition validation, and error types.
//!
//! Provides the core state machine for tasks in the educational Micro-OS,
//! guaranteeing that only legal state transitions occur and rejecting
//! invalid or unsafe state changes.

use core::fmt;

/// The life-cycle execution states of a kernel task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// The task is created and eligible to be scheduled on the CPU.
    Ready,
    /// The task is currently executing on the CPU core.
    Running,
    /// The task cannot run because it is waiting for a resource, lock, or event.
    Blocked,
    /// The task has finished execution or was explicitly terminated.
    Terminated,
}

impl TaskState {
    /// Returns the human-readable display label for this state.
    pub const fn as_str(&self) -> &'static str {
        match self {
            TaskState::Ready => "READY",
            TaskState::Running => "RUNNING",
            TaskState::Blocked => "BLOCKED",
            TaskState::Terminated => "TERMINATED",
        }
    }

    /// Validates whether transitioning from `self` to `target` is legally permitted.
    ///
    /// # Transition Rules
    /// - **Ready -> Running**: Task is selected and dispatched by the scheduler.
    /// - **Ready -> Terminated**: Task is cancelled before being scheduled.
    /// - **Running -> Ready**: Task yields or its time quantum expires (preemption).
    /// - **Running -> Blocked**: Task requests an unavailable resource or lock.
    /// - **Running -> Terminated**: Task completes execution or calls exit.
    /// - **Blocked -> Ready**: Resource becomes available or event occurs.
    /// - **Blocked -> Terminated**: Blocked task is aborted or killed.
    ///
    /// # Illegal Transitions (Rejected)
    /// - **Terminated -> Any**: A terminated task is in a terminal state and cannot be revived.
    /// - **Blocked -> Running**: Must transition to `Ready` first to wait in the ready queue.
    /// - **Ready -> Blocked**: A task cannot block without executing on the CPU first.
    pub const fn can_transition_to(&self, target: TaskState) -> bool {
        match (*self, target) {
            // Identity transition (no-op) is permitted
            (TaskState::Ready, TaskState::Ready) => true,
            (TaskState::Running, TaskState::Running) => true,
            (TaskState::Blocked, TaskState::Blocked) => true,
            (TaskState::Terminated, TaskState::Terminated) => true,

            // Legal forward transitions
            (TaskState::Ready, TaskState::Running) => true,
            (TaskState::Ready, TaskState::Terminated) => true,

            (TaskState::Running, TaskState::Ready) => true,
            (TaskState::Running, TaskState::Blocked) => true,
            (TaskState::Running, TaskState::Terminated) => true,

            (TaskState::Blocked, TaskState::Ready) => true,
            (TaskState::Blocked, TaskState::Terminated) => true,

            // All other transitions are forbidden
            _ => false,
        }
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Error returned when an illegal task state transition is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTransitionError {
    /// The state the task was currently in.
    pub from: TaskState,
    /// The requested illegal target state.
    pub to: TaskState,
}

impl fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Illegal state transition from [{}] to [{}]",
            self.from, self.to
        )
    }
}
