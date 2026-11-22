//! Task abstraction, execution states, lifecycle transitions, Task Manager, Ready Queue,
//! Process Management, and Sleep Queue.

pub mod context;
pub mod manager;
pub mod milestone12_tests;
pub mod milestone13_tests;
pub mod milestone14_tests;
pub mod process;
pub mod ready_queue;
pub mod sleep;
pub mod state;
pub mod task;
pub mod tests;

pub use context::TaskContext;
pub use manager::{
    create, create_user_task, create_with_priority, reap_terminated_tasks, terminate, transition, TaskManager,
    TaskManagerError, MAX_TASKS, TASK_MANAGER,
};
pub use process::{
    attach_thread, create_process, reap_process, sys_ipc_recv, sys_ipc_send,
    IpcError, IpcMailbox, IpcMessage, ProcessDescriptor, ProcessError, ProcessId,
    ProcessRegistry, ProcessState, RegistrySlot, MAX_PROCESSES, PROCESS_REGISTRY,
};
pub use ready_queue::{
    dequeue_task, enqueue_task, peek_task, remove_task, CancelError, CommitError,
    QueueError, QueueReservation, ReadyQueue, ReservationEntry, ReservationId,
    MAX_RESERVATIONS, QUEUE_CAPACITY, READY_QUEUE,
};
pub use sleep::{
    check_wakeups, sys_sleep, SleepEntry, SleepError, SleepQueue, MAX_SLEEPING_TASKS, SLEEP_QUEUE,
};
pub use state::{StateTransitionError, TaskState};
pub use task::{Priority, Task, TaskFn, TaskId};
