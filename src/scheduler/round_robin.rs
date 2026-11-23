//! Cooperative Round Robin Scheduler implementation.
//!
//! Enforces FIFO task selection, voluntary CPU yielding, state-machine synchronization,
//! and clean task termination with safe return to the kernel main thread.
//!
//! All kernel spinlocks are guaranteed to be released BEFORE any context switch occurs,
//! preventing deadlocks between cooperating tasks.

use spin::Mutex;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::task::manager::{TaskManager, TASK_MANAGER};
use crate::task::ready_queue::{ReadyQueue, READY_QUEUE};
use crate::task::state::TaskState;
use crate::task::task::{TaskFn, TaskId};
use crate::scheduler::context::{
    switch_context, switch_main_to_preemptive, switch_to_preemptive_task,
    return_to_main_from_preemptive, KERNEL_MAIN_RSP, SAVED_RSP,
};
use crate::{info, warn};

/// Global synchronized Round Robin Scheduler instance.
pub static SCHEDULER: Mutex<RoundRobinScheduler> = Mutex::new(RoundRobinScheduler::new());

/// Global flag indicating whether timer-driven preemption is active.
pub static PREEMPTION_ENABLED: AtomicBool = AtomicBool::new(false);

/// Total number of timer-driven preemptive context switches performed.
pub static PREEMPTION_SWITCH_COUNT: AtomicU64 = AtomicU64::new(0);

/// Saved RIP of the first task interrupted by the timer (for architectural audit).
pub static FIRST_INTERRUPTED_RIP: AtomicU64 = AtomicU64::new(0);

/// Saved RSP of the first task interrupted by the timer (for architectural audit).
pub static FIRST_INTERRUPTED_RSP: AtomicU64 = AtomicU64::new(0);

/// Cooperative Round Robin Scheduler state.
pub struct RoundRobinScheduler {
    /// Currently running TaskId on the CPU (None if on kernel main thread).
    pub current: Option<TaskId>,
    /// Total number of voluntary CPU context switches performed.
    pub switch_count: u64,
    /// Total number of times `sys_yield()` was invoked.
    pub yield_count: u64,
}

impl RoundRobinScheduler {
    /// Creates a new uninitialized RoundRobinScheduler instance.
    pub const fn new() -> Self {
        Self {
            current: None,
            switch_count: 0,
            yield_count: 0,
        }
    }

    /// Resets the scheduler state for tests or re-initialization.
    pub fn reset(&mut self) {
        self.current = None;
        self.switch_count = 0;
        self.yield_count = 0;
    }

    /// Returns the currently running TaskId, if any.
    pub fn current_task(&self) -> Option<TaskId> {
        self.current
    }

    /// Selects the next Ready task in FIFO order from the ReadyQueue.
    pub fn select_next_task(&mut self) -> Option<TaskId> {
        crate::task::dequeue_task()
    }

    /// Prepares state transitions and returns context-switch parameters for `sys_yield`.
    ///
    /// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
    /// Called with `tm`, `rq`, and `self` already locked in ascending hierarchical order.
    pub fn prepare_yield(
        &mut self,
        tm: &mut TaskManager,
        rq: &mut ReadyQueue,
    ) -> Option<(usize, u64)> {
        self.yield_count += 1;

        let curr_id = match self.current {
            Some(id) => id,
            None => {
                warn!("sys_yield called without an active running task.");
                return None;
            }
        };

        // If no other task is ready, the current task retains the CPU without transition
        if rq.is_empty() {
            return None;
        }

        // Validate that current task has a valid task table slot
        let curr_slot = match tm.get_task_slot(curr_id) {
            Some(s) => s,
            None => {
                crate::error!("sys_yield: current task {} slot not found", curr_id);
                return None;
            }
        };

        // Transition current task from RUNNING to READY (auto-appended to queue tail)
        if let Err(e) = tm.transition_task_sync(curr_id, TaskState::Ready, rq) {
            crate::error!("sys_yield: failed to transition {} to READY: {:?}", curr_id, e);
            return None;
        }

        // Dequeue the next task from the head of the Ready Queue
        let next_id = match rq.dequeue() {
            Some(id) => id,
            None => {
                // Transactional rollback: restore current task to RUNNING (auto-removed from Ready Queue)
                if let Err(rb) = tm.transition_task_sync(curr_id, TaskState::Running, rq) {
                    crate::error!("FATAL: sys_yield rollback failed restoring {} to RUNNING: {:?}. Halting CPU.", curr_id, rb);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
                return None;
            }
        };

        // Transition the next task from READY to RUNNING
        if let Err(e) = tm.transition_task_sync(next_id, TaskState::Running, rq) {
            crate::error!("sys_yield: failed to transition next task {} to RUNNING: {:?}", next_id, e);
            // Transactional rollback:
            // 1. Re-enqueue next_id so it is not lost from the Ready Queue
            if let Err(re) = rq.enqueue_raw(next_id) {
                crate::error!("FATAL: sys_yield failed to re-enqueue next task {}: {:?}. Halting CPU.", next_id, re);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            // 2. Restore current task to RUNNING
            if let Err(rb) = tm.transition_task_sync(curr_id, TaskState::Running, rq) {
                crate::error!("FATAL: sys_yield rollback failed restoring {} to RUNNING: {:?}. Halting CPU.", curr_id, rb);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            return None;
        }

        let next_slot = match tm.get_task_slot(next_id) {
            Some(s) => s,
            None => {
                crate::error!("sys_yield: next task {} slot not found", next_id);
                // Rollback both tasks
                if let Err(rb_next) = tm.transition_task_sync(next_id, TaskState::Ready, rq) {
                    crate::error!("FATAL: sys_yield failed restoring next task {} to READY: {:?}. Halting CPU.", next_id, rb_next);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
                if let Err(rb_curr) = tm.transition_task_sync(curr_id, TaskState::Running, rq) {
                    crate::error!("FATAL: sys_yield failed restoring current task {} to RUNNING: {:?}. Halting CPU.", curr_id, rb_curr);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
                return None;
            }
        };

        let next_rsp = unsafe { SAVED_RSP[next_slot] };
        if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
            crate::error!("sys_yield: next task {} has invalid RSP=0x{:x} for slot {}", next_id, next_rsp, next_slot);
            if let Err(rb_next) = tm.transition_task_sync(next_id, TaskState::Ready, rq) {
                crate::error!("FATAL: sys_yield failed restoring next task {} to READY: {:?}. Halting CPU.", next_id, rb_next);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            if let Err(rb_curr) = tm.transition_task_sync(curr_id, TaskState::Running, rq) {
                crate::error!("FATAL: sys_yield failed restoring current task {} to RUNNING: {:?}. Halting CPU.", curr_id, rb_curr);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            return None;
        }

        self.current = Some(next_id);
        self.switch_count += 1;
        Some((curr_slot, next_rsp))
    }

    /// Prepares state transitions and returns the next stack pointer for `sys_exit`.
    ///
    /// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
    /// Called with `tm`, `rq`, and `self` already locked in ascending hierarchical order.
    pub fn prepare_exit(
        &mut self,
        tm: &mut TaskManager,
        rq: &mut ReadyQueue,
    ) -> Option<u64> {
        let curr_id = match self.current {
            Some(id) => id,
            None => {
                warn!("sys_exit called without an active running task.");
                return None;
            }
        };

        let curr_slot = tm.get_task_slot(curr_id);

        // Transition current task to TERMINATED
        if let Err(e) = tm.transition_task_sync(curr_id, TaskState::Terminated, rq) {
            crate::error!("FATAL: sys_exit failed to terminate current task {}: {:?}. Halting CPU.", curr_id, e);
            loop {
                x86_64::instructions::hlt();
            }
        }

        // Detach saved RSP for terminating task
        if let Some(slot) = curr_slot {
            unsafe {
                SAVED_RSP[slot] = 0;
            }
        }

        // Select next runnable task from Ready Queue
        if let Some(next_id) = rq.dequeue() {
            // Verify task state is genuinely READY before dispatching
            let is_ready = tm.get_task(next_id).map_or(false, |task| task.state == TaskState::Ready);
            let next_slot_opt = tm.get_task_slot(next_id);

            if !is_ready {
                crate::error!("FATAL: sys_exit: dequeued task {} is not in READY state. Halting CPU.", next_id);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            let next_slot = match next_slot_opt {
                Some(s) => s,
                None => {
                    crate::error!("FATAL: sys_exit: slot for task {} not found in TaskManager. Halting CPU.", next_id);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
            };

            let next_rsp = unsafe { SAVED_RSP[next_slot] };
            if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
                crate::error!("FATAL: sys_exit: task {} has invalid saved RSP=0x{:x} for slot {}. Halting CPU.", next_id, next_rsp, next_slot);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            if let Err(e) = tm.transition_task_sync(next_id, TaskState::Running, rq) {
                crate::error!("sys_exit: failed to transition {} to RUNNING: {:?}", next_id, e);
                // Rollback: re-enqueue next_id so it is not lost from the Ready Queue
                if let Err(re) = rq.enqueue_raw(next_id) {
                    crate::error!("FATAL: sys_exit failed to re-enqueue task {}: {:?}. Halting CPU.", next_id, re);
                }
                crate::error!("FATAL: sys_exit: ready task {} failed transition to RUNNING. Halting CPU.", next_id);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            self.current = Some(next_id);
            self.switch_count += 1;
            Some(next_rsp)
        } else {
            // No runnable ready tasks remain: return to kernel main thread
            self.current = None;
            let main_rsp = unsafe { KERNEL_MAIN_RSP };
            if main_rsp != 0 {
                Some(main_rsp)
            } else {
                None
            }
        }
    }

    /// Prepares the scheduler to start from `kernel_main`.
    ///
    /// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
    /// Called with `tm`, `rq`, and `self` already locked in ascending hierarchical order.
    pub fn prepare_start(
        &mut self,
        tm: &mut TaskManager,
        rq: &mut ReadyQueue,
    ) -> Option<u64> {
        let next_id = match rq.dequeue() {
            Some(id) => id,
            None => {
                info!("Scheduler::start: Ready Queue is empty, nothing to schedule.");
                return None;
            }
        };

        if let Err(e) = tm.transition_task_sync(next_id, TaskState::Running, rq) {
            crate::error!("Scheduler::start: failed to transition {} to RUNNING: {:?}", next_id, e);
            if let Err(re) = rq.enqueue_raw(next_id) {
                crate::error!("FATAL: Scheduler::start failed to re-enqueue {}: {:?}. Halting CPU.", next_id, re);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            return None;
        }

        let next_slot = match tm.get_task_slot(next_id) {
            Some(s) => s,
            None => {
                crate::error!("Scheduler::start: slot for {} not found", next_id);
                if let Err(rb) = tm.transition_task_sync(next_id, TaskState::Ready, rq) {
                    crate::error!("FATAL: Scheduler::start rollback failed for {}: {:?}. Halting CPU.", next_id, rb);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
                return None;
            }
        };

        let next_rsp = unsafe { SAVED_RSP[next_slot] };
        if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
            crate::error!("Scheduler::start: task {} has invalid RSP=0x{:x} for slot {}", next_id, next_rsp, next_slot);
            if let Err(rb) = tm.transition_task_sync(next_id, TaskState::Ready, rq) {
                crate::error!("FATAL: Scheduler::start rollback failed for {}: {:?}. Halting CPU.", next_id, rb);
                loop {
                    x86_64::instructions::hlt();
                }
            }
            return None;
        }

        self.current = Some(next_id);
        self.switch_count += 1;
        Some(next_rsp)
    }
}

/// Initializes the global cooperative scheduler.
pub fn init() {
    SCHEDULER.lock().reset();
}

/// High-level public cooperative yield entry point.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
/// Releases all spinlocks before performing the CPU context switch.
pub fn sys_yield() {
    let switch_data = {
        let mut tm = TASK_MANAGER.lock();
        let mut rq = READY_QUEUE.lock();
        let mut sched = SCHEDULER.lock();
        let res = sched.prepare_yield(&mut tm, &mut rq);
        let cr3 = sched.current.and_then(|id| tm.get_task(id)).and_then(|t| t.cr3);
        let next_slot = sched.current.and_then(|id| tm.get_task_slot(id));
        res.map(|(slot, rsp)| (slot, rsp, cr3, next_slot))
    };

    if let Some((curr_slot, next_rsp, next_cr3, next_slot)) = switch_data {
        if let Some(cr3) = next_cr3 {
            unsafe {
                let (curr, flags) = x86_64::registers::control::Cr3::read();
                if curr != cr3 {
                    x86_64::registers::control::Cr3::write(cr3, flags);
                }
            }
        } else {
            crate::memory::address_space::restore_kernel_address_space();
        }

        if let Some(slot) = next_slot {
            unsafe {
                crate::interrupts::gdt::set_kernel_stack(
                    x86_64::VirtAddr::new(crate::scheduler::context::TASK_STACKS[slot].top())
                );
            }
        }

        unsafe {
            let old_ptr = core::ptr::addr_of_mut!(SAVED_RSP[curr_slot]);
            switch_context(old_ptr, next_rsp);
        }
    }
}

/// High-level public task exit entry point.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
/// Handles both cooperative task termination (returning via `switch_context`)
/// and preemptive task termination (switching via `iretq` or returning to `kernel_main`).
pub fn sys_exit() {
    if PREEMPTION_ENABLED.load(Ordering::SeqCst) {
        // Preemptive task exit: disable interrupts to protect state transition
        x86_64::instructions::interrupts::disable();

        let exit_data = {
            let mut tm = TASK_MANAGER.lock();
            let mut rq = READY_QUEUE.lock();
            let mut sched = SCHEDULER.lock();
            let res = sched.prepare_exit(&mut tm, &mut rq);
            let cr3 = sched.current.and_then(|id| tm.get_task(id)).and_then(|t| t.cr3);
            let next_slot = sched.current.and_then(|id| tm.get_task_slot(id));
            res.map(|rsp| (rsp, cr3, next_slot))
        };

        if let Some((next_rsp, next_cr3, next_slot)) = exit_data {
            let main_rsp = unsafe { KERNEL_MAIN_RSP };
            if next_rsp == main_rsp {
                // All preemptive tasks completed: restore kernel CR3 and return cleanly to kernel main thread
                crate::memory::address_space::restore_kernel_address_space();
                crate::interrupts::gdt::restore_default_kernel_stack();
                PREEMPTION_ENABLED.store(false, Ordering::SeqCst);
                unsafe {
                    return_to_main_from_preemptive(main_rsp);
                }
            } else {
                if let Some(cr3) = next_cr3 {
                    unsafe {
                        let (curr, flags) = x86_64::registers::control::Cr3::read();
                        if curr != cr3 {
                            x86_64::registers::control::Cr3::write(cr3, flags);
                        }
                    }
                } else {
                    crate::memory::address_space::restore_kernel_address_space();
                }

                if let Some(slot) = next_slot {
                    unsafe {
                        crate::interrupts::gdt::set_kernel_stack(
                            x86_64::VirtAddr::new(crate::scheduler::context::TASK_STACKS[slot].top())
                        );
                    }
                }

                // Switch to next ready preemptive task via iretq
                unsafe {
                    switch_to_preemptive_task(next_rsp);
                }
            }
        } else {
            loop {
                x86_64::instructions::hlt();
            }
        }
    } else {
        // Milestone 8 cooperative task exit
        let exit_data = {
            let mut tm = TASK_MANAGER.lock();
            let mut rq = READY_QUEUE.lock();
            let mut sched = SCHEDULER.lock();
            let res = sched.prepare_exit(&mut tm, &mut rq);
            let cr3 = sched.current.and_then(|id| tm.get_task(id)).and_then(|t| t.cr3);
            let next_slot = sched.current.and_then(|id| tm.get_task_slot(id));
            res.map(|rsp| (rsp, cr3, next_slot))
        };

        if let Some((next_rsp, next_cr3, next_slot)) = exit_data {
            if let Some(cr3) = next_cr3 {
                unsafe {
                    let (curr, flags) = x86_64::registers::control::Cr3::read();
                    if curr != cr3 {
                        x86_64::registers::control::Cr3::write(cr3, flags);
                    }
                }
            } else {
                crate::memory::address_space::restore_kernel_address_space();
            }

            if let Some(slot) = next_slot {
                unsafe {
                    crate::interrupts::gdt::set_kernel_stack(
                        x86_64::VirtAddr::new(crate::scheduler::context::TASK_STACKS[slot].top())
                    );
                }
            } else {
                crate::interrupts::gdt::restore_default_kernel_stack();
            }

            static mut EXIT_DUMMY_RSP: u64 = 0;
            unsafe {
                switch_context(core::ptr::addr_of_mut!(EXIT_DUMMY_RSP), next_rsp);
            }
        } else {
            loop {
                x86_64::instructions::hlt();
            }
        }
    }
}

/// Starts timer-driven preemptive Round Robin scheduling from `kernel_main`.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
/// Enables timer preemption and switches to the first task via `iretq`.
/// Execution returns to the call site once all tasks have terminated.
pub fn start_preemptive_scheduling() -> bool {
    let (first_rsp, first_slot) = {
        let mut tm = TASK_MANAGER.lock();
        let mut rq = READY_QUEUE.lock();
        let mut sched = SCHEDULER.lock();
        match sched.prepare_start(&mut tm, &mut rq) {
            Some(rsp) => {
                let slot = sched.current.and_then(|id| tm.get_task_slot(id));
                (rsp, slot)
            }
            None => {
                crate::warn!("start_preemptive_scheduling: no ready tasks to schedule.");
                return false;
            }
        }
    };

    if let Some(slot) = first_slot {
        unsafe {
            crate::interrupts::gdt::set_kernel_stack(
                x86_64::VirtAddr::new(crate::scheduler::context::TASK_STACKS[slot].top())
            );
        }
    }

    PREEMPTION_ENABLED.store(true, Ordering::SeqCst);
    unsafe {
        let main_ptr = core::ptr::addr_of_mut!(KERNEL_MAIN_RSP);
        switch_main_to_preemptive(main_ptr, first_rsp);
        KERNEL_MAIN_RSP = 0;
    }
    true
}

/// C-ABI entry point called from `timer_interrupt_entry` naked assembly.
#[no_mangle]
pub extern "C" fn timer_interrupt_handler(rsp: u64) -> u64 {
    // 1. Monotonically increment hardware timer ticks
    let current_tick = crate::timer::increment_ticks();

    // 2. Poll SleepQueue for sleeping tasks that have reached their target tick
    crate::task::check_wakeups(current_tick);

    // 3. Acknowledge interrupt by sending EOI to 8259 PIC
    crate::interrupts::pic::send_eoi(crate::interrupts::pic::TIMER_INTERRUPT_ID);

    // 4. If preemption is active, attempt preemptive scheduling
    if PREEMPTION_ENABLED.load(Ordering::SeqCst) {
        handle_timer_preemption(rsp)
    } else {
        rsp
    }
}

/// Core preemptive scheduling decision routine invoked on timer interrupts.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
/// Uses `try_lock()` on all kernel mutexes in hierarchical order to guarantee zero deadlock risk.
/// If any lock is contested or no other task is ready, the current task retains the CPU.
pub fn handle_timer_preemption(rsp: u64) -> u64 {
    let mut tm = match crate::task::manager::TASK_MANAGER.try_lock() {
        Some(m) => m,
        None => return rsp,
    };

    let mut rq = match crate::task::ready_queue::READY_QUEUE.try_lock() {
        Some(q) => q,
        None => return rsp,
    };

    if rq.is_empty() {
        return rsp;
    }

    let mut sched = match SCHEDULER.try_lock() {
        Some(s) => s,
        None => return rsp,
    };

    let curr_id = match sched.current {
        Some(id) => id,
        None => return rsp,
    };

    let curr_slot = match tm.get_task_slot(curr_id) {
        Some(s) => s,
        None => return rsp,
    };

    // Save current task's interrupted RSP
    unsafe {
        SAVED_RSP[curr_slot] = rsp;
    }

    // Transition current task Running -> Ready (auto-appended to ReadyQueue tail)
    if let Err(e) = tm.transition_task_sync(curr_id, TaskState::Ready, &mut rq) {
        crate::error!("handle_timer_preemption: failed transition {} to READY: {:?}", curr_id, e);
        return rsp;
    }

    // Dequeue next task from ReadyQueue
    let next_id = match rq.dequeue() {
        Some(id) => id,
        None => {
            let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
            return rsp;
        }
    };

    // Transition next task Ready -> Running
    if let Err(e) = tm.transition_task_sync(next_id, TaskState::Running, &mut rq) {
        crate::error!("handle_timer_preemption: failed transition {} to RUNNING: {:?}", next_id, e);
        let _ = rq.enqueue_raw(next_id);
        let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
        return rsp;
    }

    let next_slot = match tm.get_task_slot(next_id) {
        Some(s) => s,
        None => {
            let _ = tm.transition_task_sync(next_id, TaskState::Ready, &mut rq);
            let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
            return rsp;
        }
    };

    let next_rsp = unsafe { SAVED_RSP[next_slot] };
    if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
        crate::error!("handle_timer_preemption: invalid RSP=0x{:x} for slot {}", next_rsp, next_slot);
        let _ = tm.transition_task_sync(next_id, TaskState::Ready, &mut rq);
        let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
        return rsp;
    }

    sched.current = Some(next_id);
    sched.switch_count += 1;
    let count = PREEMPTION_SWITCH_COUNT.fetch_add(1, Ordering::SeqCst) + 1;

    // Switch hardware address space (CR3) and update TSS kernel privilege stack
    let next_cr3 = tm.get_task(next_id).and_then(|t| t.cr3);
    if let Some(cr3) = next_cr3 {
        unsafe {
            let (curr, flags) = x86_64::registers::control::Cr3::read();
            if curr != cr3 {
                x86_64::registers::control::Cr3::write(cr3, flags);
            }
        }
    } else {
        crate::memory::address_space::restore_kernel_address_space();
    }

    unsafe {
        crate::interrupts::gdt::set_kernel_stack(x86_64::VirtAddr::new(crate::scheduler::context::TASK_STACKS[next_slot].top()));
    }

    // Read hardware interrupt frame words pushed by the CPU at [rsp + 120..160]
    let rip = unsafe { *(rsp as *const u64).add(15) };
    let cs = unsafe { *(rsp as *const u64).add(16) };
    let rflags = unsafe { *(rsp as *const u64).add(17) };
    let task_rsp = unsafe { *(rsp as *const u64).add(18) };
    let ss = unsafe { *(rsp as *const u64).add(19) };

    if count == 1 {
        FIRST_INTERRUPTED_RIP.store(rip, Ordering::SeqCst);
        FIRST_INTERRUPTED_RSP.store(task_rsp, Ordering::SeqCst);
        crate::info!("  [HW-FRAME AUDIT] Interrupted Ring 0 Hardware Stack Frame:");
        crate::info!("    [RSP + 120] Saved RIP:    0x{:016x}", rip);
        crate::info!("    [RSP + 128] Saved CS:     0x{:016x} (CPL=0)", cs);
        crate::info!("    [RSP + 136] Saved RFLAGS: 0x{:016x} (IF={})", rflags, (rflags >> 9) & 1);
        crate::info!("    [RSP + 144] Saved RSP:    0x{:016x}", task_rsp);
        crate::info!("    [RSP + 152] Saved SS:     0x{:016x}", ss);
    }

    if count <= 8 {
        crate::info!(
            "  [TIMER IRQ #{}] Preempted {} -> Dispatched {} (tick: {})",
            count,
            curr_id,
            next_id,
            crate::timer::get_ticks()
        );
    }

    next_rsp
}

/// Starts cooperative Round Robin scheduling from `kernel_main`.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> READY_QUEUE (Level 4) -> SCHEDULER (Level 5).
/// Execution returns to the call site once all tasks have terminated.
pub fn start() {
    let start_data = {
        let mut tm = TASK_MANAGER.lock();
        let mut rq = READY_QUEUE.lock();
        let mut sched = SCHEDULER.lock();
        sched.prepare_start(&mut tm, &mut rq)
    };

    if let Some(first_rsp) = start_data {
        unsafe {
            let main_ptr = core::ptr::addr_of_mut!(KERNEL_MAIN_RSP);
            switch_context(main_ptr, first_rsp);
            KERNEL_MAIN_RSP = 0;
        }
    }
}

/// Returns the currently running TaskId, if any.
pub fn current_task() -> Option<TaskId> {
    SCHEDULER.lock().current_task()
}

/// Selects the next Ready task in FIFO order from the ReadyQueue.
pub fn select_next_task() -> Option<TaskId> {
    READY_QUEUE.lock().dequeue()
}

/// Retrieves the entry point function of the currently running task.
///
/// Observes strict global lock hierarchy: TASK_MANAGER (Level 3) -> SCHEDULER (Level 5).
pub fn current_task_entry() -> Option<TaskFn> {
    let tm = TASK_MANAGER.lock();
    let sched = SCHEDULER.lock();
    let curr_id = sched.current?;
    tm.get_task(curr_id).and_then(|t| t.entry_point)
}
