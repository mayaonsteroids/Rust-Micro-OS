//! Execution context representation for task state frames.

/// Minimal architectural CPU context structure for task state representation.
///
/// Holds the critical register values required to save and restore an execution thread:
/// - `rsp`: Stack Pointer (top of task's private stack)
/// - `rbp`: Base Pointer (frame base for stack debugging)
/// - `rip`: Instruction Pointer (task entry point / resume address)
/// - `rflags`: CPU Status Flags (including the interrupt enable flag IF)
///
/// NOTE: Full hardware register save/restore and preemptive stack swapping are
/// connected during the Scheduler and Timer milestones (Milestones 8 and 9).
/// In Milestone 6, this structure provides the baseline execution context frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbp: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl TaskContext {
    /// Creates a newly initialized CPU context frame.
    ///
    /// Sets `rflags` to `0x202` (x86_64 default: bit 1 reserved, bit 9 IF enabled).
    pub const fn new() -> Self {
        Self {
            rsp: 0,
            rbp: 0,
            rip: 0,
            rflags: 0x202,
        }
    }
}
