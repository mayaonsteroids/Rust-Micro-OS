//! Task Manager: fixed-capacity task registry, lookup, lifecycle transitions, and logging.

use core::fmt;
use spin::Mutex;
use crate::task::ready_queue::{QueueError, ReadyQueue, READY_QUEUE};
use crate::task::state::{StateTransitionError, TaskState};
use crate::task::task::{Priority, Task, TaskFn, TaskId};

/// Maximum number of tasks supported simultaneously by the kernel task table.
pub const MAX_TASKS: usize = 32;

/// Error conditions encountered during task management operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskManagerError {
    /// The fixed-capacity task table is full; no slots available.
    TableFull,
    /// The specified TaskId does not exist in the task table.
    TaskNotFound(TaskId),
    /// An illegal task state transition was attempted.
    InvalidTransition(StateTransitionError),
    /// A Ready Queue error occurred during state transition synchronization.
    QueueError(QueueError),
}

impl fmt::Display for TaskManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskManagerError::TableFull => write!(f, "Task table full (capacity: {})", MAX_TASKS),
            TaskManagerError::TaskNotFound(id) => write!(f, "Task not found with ID {}", id),
            TaskManagerError::InvalidTransition(err) => write!(f, "{}", err),
            TaskManagerError::QueueError(err) => write!(f, "Queue synchronization error: {}", err),
        }
    }
}

/// Fixed-capacity Task Manager storing Task Control Blocks without heap allocation.
pub struct TaskManager {
    tasks: [Option<Task>; MAX_TASKS],
    count: usize,
}

impl TaskManager {
    /// Creates a new empty TaskManager with all slots vacant.
    pub const fn new() -> Self {
        Self {
            tasks: [None; MAX_TASKS],
            count: 0,
        }
    }

    /// Registers a new task in the table and returns its unique TaskId.
    pub fn create_task(
        &mut self,
        name: &'static str,
        entry_point: Option<TaskFn>,
    ) -> Result<TaskId, TaskManagerError> {
        self.create_task_with_priority(name, Priority::Normal, entry_point)
    }

    /// Registers a new task with an explicit priority.
    pub fn create_task_with_priority(
        &mut self,
        name: &'static str,
        priority: Priority,
        entry_point: Option<TaskFn>,
    ) -> Result<TaskId, TaskManagerError> {
        for (i, slot) in self.tasks.iter_mut().enumerate() {
            if slot.is_none() {
                let mut task = Task::new_with_priority(name, priority, entry_point);
                let id = task.id;
                let initial_rsp = unsafe {
                    crate::scheduler::context::setup_task_stack(i, entry_point)
                };
                task.context.rsp = initial_rsp;
                *slot = Some(task);
                self.count += 1;
                return Ok(id);
            }
        }
        Err(TaskManagerError::TableFull)
    }

    /// Registers a new user-mode task with an associated Level 4 Page Table (CR3).
    pub fn create_user_task(
        &mut self,
        name: &'static str,
        priority: Priority,
        entry_point: Option<TaskFn>,
        cr3: x86_64::structures::paging::PhysFrame,
    ) -> Result<TaskId, TaskManagerError> {
        let tid = self.create_task_with_priority(name, priority, entry_point)?;
        if let Some(task) = self.get_task_mut(tid) {
            task.cr3 = Some(cr3);
        }
        Ok(tid)
    }

    /// Retrieves the internal task table slot index for a given TaskId.
    pub fn get_task_slot(&self, id: TaskId) -> Option<usize> {
        self.tasks.iter().position(|slot| {
            slot.as_ref().map_or(false, |t| t.id == id)
        })
    }

    /// Retrieves an immutable reference to a task by its TaskId.
    pub fn get_task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.iter().find_map(|slot| {
            if let Some(task) = slot {
                if task.id == id {
                    return Some(task);
                }
            }
            None
        })
    }

    /// Retrieves a mutable reference to a task by its TaskId.
    pub fn get_task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.iter_mut().find_map(|slot| {
            if let Some(task) = slot {
                if task.id == id {
                    return Some(task);
                }
            }
            None
        })
    }

    /// Changes a task's state, enforcing valid state transition rules, and
    /// automatically synchronizes with the supplied Ready Queue.
    ///
    /// Atomic guarantee: if the transition or queue insertion fails (e.g. queue full),
    /// both the task state and the Ready Queue remain strictly unchanged.
    pub fn transition_task_sync(
        &mut self,
        id: TaskId,
        new_state: TaskState,
        rq: &mut ReadyQueue,
    ) -> Result<(), TaskManagerError> {
        let task = self.get_task_mut(id).ok_or(TaskManagerError::TaskNotFound(id))?;
        let old_state = task.state;

        // Step 1: Validate legal state transition first
        if !task.state.can_transition_to(new_state) {
            return Err(TaskManagerError::InvalidTransition(StateTransitionError {
                from: old_state,
                to: new_state,
            }));
        }

        // Step 2: If transitioning into Ready, check queue capacity and duplicates BEFORE changing task state
        if new_state == TaskState::Ready && old_state != TaskState::Ready {
            if rq.is_full() {
                return Err(TaskManagerError::QueueError(QueueError::QueueFull));
            }
            if rq.contains(id) {
                return Err(TaskManagerError::QueueError(QueueError::DuplicateTask(id)));
            }
        }

        // Step 3: Transition validated; update the task's state
        task.transition_to(new_state)
            .map_err(TaskManagerError::InvalidTransition)?;

        // Step 4: Perform the synchronized queue update
        match (old_state, new_state) {
            // Moving into Ready from non-Ready: enqueue at tail
            (_, TaskState::Ready) => {
                if !rq.contains(id) {
                    if let Err(e) = rq.enqueue_raw(id) {
                        // Rollback task state if enqueue fails
                        task.state = old_state;
                        return Err(TaskManagerError::QueueError(e));
                    }
                }
            }
            // Moving out of Ready: remove from queue
            (TaskState::Ready, _) => {
                rq.remove(id);
            }
            // Non-ready transitions (e.g. Running -> Blocked, Running -> Terminated): ensure not in queue
            _ => {
                rq.remove(id);
            }
        }

        Ok(())
    }

    /// Changes a task's state, enforcing valid state transition rules, and
    /// automatically synchronizes with the global Ready Queue.
    pub fn transition_task(
        &mut self,
        id: TaskId,
        new_state: TaskState,
    ) -> Result<(), TaskManagerError> {
        let mut rq = READY_QUEUE.lock();
        self.transition_task_sync(id, new_state, &mut rq)
    }

    /// Terminates a task by transitioning it to `TERMINATED` and removes it from Ready Queue.
    /// Also detaches SAVED_RSP if the CPU is not executing on the task's stack.
    pub fn terminate_task(&mut self, id: TaskId) -> Result<(), TaskManagerError> {
        self.transition_task(id, TaskState::Terminated)?;
        if let Some(slot) = self.get_task_slot(id) {
            let curr_sp = crate::scheduler::context::current_rsp();
            let stack_base = unsafe { crate::scheduler::context::TASK_STACKS[slot].data.as_ptr() as u64 };
            let stack_top = unsafe { crate::scheduler::context::TASK_STACKS[slot].top() };
            let cpu_on_stack = curr_sp >= stack_base && curr_sp < stack_top;
            if !cpu_on_stack {
                unsafe {
                    crate::scheduler::context::SAVED_RSP[slot] = 0;
                }
            }
        }
        Ok(())
    }

    /// Returns the total count of registered tasks (active + terminated).
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the count of active (non-terminated) tasks.
    pub fn active_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|slot| {
                if let Some(task) = slot {
                    !task.is_terminated()
                } else {
                    false
                }
            })
            .count()
    }

    /// Returns an iterator over all currently stored tasks.
    pub fn iter(&self) -> impl Iterator<Item = &Task> {
        self.tasks.iter().filter_map(|s| s.as_ref())
    }

    /// Reaps terminated tasks, freeing their slots for reuse.
    ///
    /// Safety Invariants Enforced:
    /// 1. Task must be in `Terminated` state.
    /// 2. Context must be detached (`SAVED_RSP[slot] == 0`).
    /// 3. Current CPU stack pointer must NOT fall within the task's stack buffer.
    pub fn reap_terminated_tasks(&mut self) -> usize {
        let mut reaped = 0;
        let curr_sp = crate::scheduler::context::current_rsp();

        for slot_idx in 0..MAX_TASKS {
            if let Some(ref task) = self.tasks[slot_idx] {
                if task.is_terminated() {
                    let saved_rsp = unsafe { crate::scheduler::context::SAVED_RSP[slot_idx] };
                    let stack_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_idx].data.as_ptr() as u64 };
                    let stack_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_idx].top() };

                    // Verify CPU is not executing on this stack
                    let cpu_on_stack = curr_sp >= stack_base && curr_sp < stack_top;

                    // Reclaim only if task is terminated, CPU is not running on this stack,
                    // and context has detached (saved_rsp == 0)
                    if !cpu_on_stack && saved_rsp == 0 {
                        self.tasks[slot_idx] = None;
                        self.count = self.count.saturating_sub(1);
                        reaped += 1;
                    }
                }
            }
        }
        reaped
    }
}

/// Global synchronized TaskManager instance.
pub static TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

/// Public helper: creates a new task in the global task manager.
pub fn create(name: &'static str, entry_point: Option<TaskFn>) -> Result<TaskId, TaskManagerError> {
    TASK_MANAGER.lock().create_task(name, entry_point)
}

/// Public helper: creates a new task with a specified priority.
pub fn create_with_priority(
    name: &'static str,
    priority: Priority,
    entry_point: Option<TaskFn>,
) -> Result<TaskId, TaskManagerError> {
    TASK_MANAGER.lock().create_task_with_priority(name, priority, entry_point)
}

/// Public helper: creates a new user-mode task with an associated Level 4 Page Table (CR3).
pub fn create_user_task(
    name: &'static str,
    priority: Priority,
    entry_point: Option<TaskFn>,
    cr3: x86_64::structures::paging::PhysFrame,
) -> Result<TaskId, TaskManagerError> {
    TASK_MANAGER.lock().create_user_task(name, priority, entry_point, cr3)
}

/// Public helper: transitions a task to a new state.
pub fn transition(id: TaskId, new_state: TaskState) -> Result<(), TaskManagerError> {
    TASK_MANAGER.lock().transition_task(id, new_state)
}

/// Public helper: terminates a task.
pub fn terminate(id: TaskId) -> Result<(), TaskManagerError> {
    TASK_MANAGER.lock().terminate_task(id)
}

/// Public helper: reaps terminated tasks and reclaims their slots.
pub fn reap_terminated_tasks() -> usize {
    TASK_MANAGER.lock().reap_terminated_tasks()
}
