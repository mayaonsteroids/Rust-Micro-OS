//! CPU scheduling algorithms, cooperative task switching, and ready queue management.

pub mod context;
pub mod ready_queue;
pub mod round_robin;
pub mod tests;

pub use round_robin::{
    current_task, current_task_entry, init, select_next_task, start,
    start_preemptive_scheduling, sys_exit, sys_yield,
    RoundRobinScheduler, PREEMPTION_ENABLED, PREEMPTION_SWITCH_COUNT, SCHEDULER,
};
