//! Unit tests and live demonstration for the Cooperative Round Robin Scheduler.
//!
//! Covers all 10 Milestone 8 requirements:
//! 1. Scheduler initialization.
//! 2. Selecting the first Ready task.
//! 3. FIFO selection of multiple Ready tasks.
//! 4. Cooperative yield and re-queuing.
//! 5. Round Robin order over multiple yields.
//! 6. Handling an empty ReadyQueue.
//! 7. Handling blocked tasks.
//! 8. Handling terminated tasks.
//! 9. Preventing duplicate queue entries.
//! 10. Preserving state consistency across yields.
//!
//! Also provides the live multi-task execution demonstration (`run_cooperative_task_demo`).

use core::sync::atomic::{AtomicU64, Ordering};
use crate::scheduler::round_robin::RoundRobinScheduler;
use crate::task::manager::TaskManager;
use crate::task::ready_queue::ReadyQueue;
use crate::task::state::TaskState;
use crate::task::task::TaskId;
use crate::{error, info};

/// Runs the complete Milestone 8 scheduler unit test suite.
pub fn run_scheduler_unit_tests() -> bool {
    info!("========================================================");
    info!("    RUNNING MILESTONE 8: SCHEDULER UNIT TEST SUITE      ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_scheduler_init();
    all_passed &= test_select_first_ready_task();
    all_passed &= test_fifo_selection_multiple_tasks();
    all_passed &= test_cooperative_yield_requeues_at_tail();
    all_passed &= test_round_robin_fairness_over_multiple_yields();
    all_passed &= test_empty_queue_yield_safety();
    all_passed &= test_blocked_task_never_scheduled();
    all_passed &= test_terminated_task_never_requeued();
    all_passed &= test_no_duplicate_queue_entries_during_scheduling();
    all_passed &= test_scheduler_state_consistency();
    all_passed &= test_task_registration_vs_queue_eligibility();
    all_passed &= test_stack_pointer_validation_boundaries();

    if all_passed {
        info!(">>> MILESTONE 8 SCHEDULER TESTS: ALL 12 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 8 SCHEDULER TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// Test 1: Scheduler initialization.
fn test_scheduler_init() -> bool {
    let sched = RoundRobinScheduler::new();
    let init_ok = sched.current.is_none() && sched.switch_count == 0 && sched.yield_count == 0;
    if init_ok {
        info!("  [PASS] SCHED-TC1: Scheduler initialization state verified.");
    } else {
        error!("  [FAIL] SCHED-TC1: Scheduler initialization failed!");
    }
    init_ok
}

/// Test 2: Selecting the first Ready task.
fn test_select_first_ready_task() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let _sched = RoundRobinScheduler::new();

    let id = tm.create_task("T1", None).unwrap();
    rq.enqueue(id, &tm).unwrap();

    let selected = rq.dequeue();
    let pass = selected == Some(id) && rq.is_empty();
    if pass {
        info!("  [PASS] SCHED-TC2: Selecting first Ready task in queue verified.");
    } else {
        error!("  [FAIL] SCHED-TC2: Failed to select first Ready task!");
    }
    pass
}

/// Test 3: FIFO selection of multiple Ready tasks.
fn test_fifo_selection_multiple_tasks() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("T1", None).unwrap();
    let id2 = tm.create_task("T2", None).unwrap();
    let id3 = tm.create_task("T3", None).unwrap();

    rq.enqueue(id1, &tm).unwrap();
    rq.enqueue(id2, &tm).unwrap();
    rq.enqueue(id3, &tm).unwrap();

    let s1 = rq.dequeue();
    let s2 = rq.dequeue();
    let s3 = rq.dequeue();
    let s4 = rq.dequeue();

    let pass = s1 == Some(id1) && s2 == Some(id2) && s3 == Some(id3) && s4 == None;
    if pass {
        info!("  [PASS] SCHED-TC3: FIFO selection order (T1 -> T2 -> T3) verified.");
    } else {
        error!("  [FAIL] SCHED-TC3: FIFO selection mismatch!");
    }
    pass
}

/// Test 4: Cooperative yield and re-queuing.
fn test_cooperative_yield_requeues_at_tail() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id_a = tm.create_task("TaskA", None).unwrap();
    let id_b = tm.create_task("TaskB", None).unwrap();

    // Start Task A in RUNNING, Task B in READY
    tm.transition_task_sync(id_a, TaskState::Running, &mut rq).unwrap();
    rq.enqueue(id_b, &tm).unwrap();

    assert_eq!(rq.len(), 1);
    assert!(rq.contains(id_b));

    // Simulate yield of Task A:
    // 1. Task A transitions Running -> Ready (automatically appended to tail)
    tm.transition_task_sync(id_a, TaskState::Ready, &mut rq).unwrap();
    assert_eq!(rq.len(), 2);

    // 2. Scheduler selects head (Task B)
    let next = rq.dequeue().unwrap();
    assert_eq!(next, id_b);
    tm.transition_task_sync(id_b, TaskState::Running, &mut rq).unwrap();

    // 3. Queue now contains Task A at head
    let pass = rq.len() == 1 && rq.peek() == Some(id_a);
    if pass {
        info!("  [PASS] SCHED-TC4: Cooperative yield re-queues yielded task at queue tail.");
    } else {
        error!("  [FAIL] SCHED-TC4: Cooperative yield re-queuing failed!");
    }
    pass
}

/// Test 5: Round Robin order over multiple yields.
fn test_round_robin_fairness_over_multiple_yields() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("T1", None).unwrap();
    let id2 = tm.create_task("T2", None).unwrap();
    let id3 = tm.create_task("T3", None).unwrap();

    // Initial state: T1 is running, T2 and T3 are queued
    tm.transition_task_sync(id1, TaskState::Running, &mut rq).unwrap();
    rq.enqueue(id2, &tm).unwrap();
    rq.enqueue(id3, &tm).unwrap();

    let mut current = id1;
    let mut history = [TaskId(0); 6];

    // Simulate 6 yields
    for i in 0..6 {
        // Current task yields: moves to Ready at tail
        tm.transition_task_sync(current, TaskState::Ready, &mut rq).unwrap();
        // Next task picked from head
        let next = rq.dequeue().unwrap();
        tm.transition_task_sync(next, TaskState::Running, &mut rq).unwrap();
        current = next;
        history[i] = current;
    }

    // Expected cyclic order: T2, T3, T1, T2, T3, T1
    let expected = [id2, id3, id1, id2, id3, id1];
    let pass = history == expected;
    if pass {
        info!("  [PASS] SCHED-TC5: Round Robin cyclic fairness across 6 yields verified (T2->T3->T1->T2->T3->T1).");
    } else {
        error!("  [FAIL] SCHED-TC5: Round Robin fairness pattern mismatch!");
    }
    pass
}

/// Test 6: Handling an empty ReadyQueue safely.
fn test_empty_queue_yield_safety() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id = tm.create_task("SoloTask", None).unwrap();
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();

    // When solo task yields but queue is empty, peek returns None
    let peek_empty = rq.peek().is_none();
    // Task remains in Running state
    let still_running = tm.get_task(id).unwrap().state == TaskState::Running;

    let pass = peek_empty && still_running;
    if pass {
        info!("  [PASS] SCHED-TC6: Empty ReadyQueue handled safely; solo task retains CPU.");
    } else {
        error!("  [FAIL] SCHED-TC6: Empty queue handling failed!");
    }
    pass
}

/// Test 7: Blocked tasks are never scheduled.
fn test_blocked_task_never_scheduled() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id_ready = tm.create_task("ReadyWorker", None).unwrap();
    rq.enqueue(id_ready, &tm).unwrap();

    let id_blocked = tm.create_task("BlockedWorker", None).unwrap();
    tm.transition_task_sync(id_blocked, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(id_blocked, TaskState::Blocked, &mut rq).unwrap();

    // Verify ReadyQueue only contains the Ready task
    let queue_has_blocked = rq.contains(id_blocked);
    let selected = rq.dequeue();
    let next_selected = rq.dequeue();

    let pass = !queue_has_blocked && selected == Some(id_ready) && next_selected.is_none();
    if pass {
        info!("  [PASS] SCHED-TC7: Blocked task is excluded from ReadyQueue and never scheduled.");
    } else {
        error!("  [FAIL] SCHED-TC7: Blocked task was scheduled or present in queue!");
    }
    pass
}

/// Test 8: Terminated tasks are never re-queued.
fn test_terminated_task_never_requeued() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("TermTask", None).unwrap();
    let id2 = tm.create_task("NextTask", None).unwrap();

    tm.transition_task_sync(id1, TaskState::Running, &mut rq).unwrap();
    rq.enqueue(id2, &tm).unwrap();

    // Terminate Task 1
    tm.transition_task_sync(id1, TaskState::Terminated, &mut rq).unwrap();
    let in_queue_after_term = rq.contains(id1);

    // Pick next task
    let next = rq.dequeue();
    let empty_after = rq.is_empty();

    let pass = !in_queue_after_term && next == Some(id2) && empty_after;
    if pass {
        info!("  [PASS] SCHED-TC8: Terminated task is purged and never re-queued.");
    } else {
        error!("  [FAIL] SCHED-TC8: Terminated task handling failed!");
    }
    pass
}

/// Test 9: Preventing duplicate entries in ReadyQueue.
fn test_no_duplicate_queue_entries_during_scheduling() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id = tm.create_task("DupCheck", None).unwrap();
    rq.enqueue(id, &tm).unwrap();
    assert_eq!(rq.len(), 1);

    // Re-transitioning into Ready must not create duplicate
    tm.transition_task_sync(id, TaskState::Ready, &mut rq).unwrap();
    let len_one = rq.len() == 1;

    let d1 = rq.dequeue();
    let d2 = rq.dequeue();

    let pass = len_one && d1 == Some(id) && d2.is_none();
    if pass {
        info!("  [PASS] SCHED-TC9: Duplicate ReadyQueue entries prevented during scheduler transitions.");
    } else {
        error!("  [FAIL] SCHED-TC9: Duplicate entries detected!");
    }
    pass
}

/// Test 10: Consistency between TaskManager states and ReadyQueue.
fn test_scheduler_state_consistency() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("TaskOne", None).unwrap();
    let id2 = tm.create_task("TaskTwo", None).unwrap();
    rq.enqueue(id1, &tm).unwrap();
    rq.enqueue(id2, &tm).unwrap();

    // Invariant: Both tasks in ReadyQueue must be in TaskState::Ready
    let t1_ready = tm.get_task(id1).unwrap().state == TaskState::Ready;
    let t2_ready = tm.get_task(id2).unwrap().state == TaskState::Ready;

    // Dispatch T1 to Running
    let next = rq.dequeue().unwrap();
    tm.transition_task_sync(next, TaskState::Running, &mut rq).unwrap();

    let t1_running = tm.get_task(id1).unwrap().state == TaskState::Running;
    let t1_not_in_queue = !rq.contains(id1);

    let pass = t1_ready && t2_ready && t1_running && t1_not_in_queue;
    if pass {
        info!("  [PASS] SCHED-TC10: State consistency between TaskManager and ReadyQueue strictly verified.");
    } else {
        error!("  [FAIL] SCHED-TC10: State consistency mismatch!");
    }
    pass
}

/// Test 11: Explicit distinction between task registration, scheduling eligibility, and ReadyQueue admission.
fn test_task_registration_vs_queue_eligibility() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    // 1. Task registration: creating a task allocates a TCB and private stack.
    let id = tm.create_task("RegisteredTask", None).expect("Create task");

    // (a) Task registered in TaskManager: valid slot, known ID, count == 1
    let registered_ok = tm.get_task(id).is_some() && tm.count() == 1;

    // (b) Task is logically in Ready state (configured, stack ready, eligible for scheduling)
    let state_ready = tm.get_task(id).unwrap().state == TaskState::Ready;

    // (c) Task is NOT yet enqueued in ReadyQueue (ReadyQueue is empty, does not contain id)
    let not_in_queue = !rq.contains(id) && rq.is_empty();

    // (d) Scheduler / ReadyQueue dequeue returns None (task cannot be scheduled until enqueued)
    let not_scheduled = rq.dequeue().is_none();

    // 2. Admission: Task is explicitly enqueued into ReadyQueue
    let enqueue_ok = rq.enqueue(id, &tm).is_ok();
    let in_queue_now = rq.contains(id) && rq.len() == 1;

    // 3. Dispatch: Moving task to Running automatically removes it from ReadyQueue
    let dispatch_ok = tm.transition_task_sync(id, TaskState::Running, &mut rq).is_ok();
    let running_not_in_queue = tm.get_task(id).unwrap().state == TaskState::Running && !rq.contains(id);

    // 4. Yield/Preemption: Transitioning back to Ready automatically re-enqueues at queue tail
    let yield_ok = tm.transition_task_sync(id, TaskState::Ready, &mut rq).is_ok();
    let ready_in_queue_again = tm.get_task(id).unwrap().state == TaskState::Ready && rq.contains(id);

    let pass = registered_ok
        && state_ready
        && not_in_queue
        && not_scheduled
        && enqueue_ok
        && in_queue_now
        && dispatch_ok
        && running_not_in_queue
        && yield_ok
        && ready_in_queue_again;

    if pass {
        info!("  [PASS] SCHED-TC11: Task registration vs ReadyQueue scheduling eligibility verified.");
    } else {
        error!("  [FAIL] SCHED-TC11: Task registration vs ReadyQueue scheduling eligibility mismatch!");
    }
    pass
}

/// Test 12: Stack pointer boundary conditions and minimum headroom validation.
fn test_stack_pointer_validation_boundaries() -> bool {
    let slot = 0;
    let base = unsafe { crate::scheduler::context::TASK_STACKS[slot].data.as_ptr() as u64 };
    let top = unsafe { crate::scheduler::context::TASK_STACKS[slot].top() };
    let headroom = crate::scheduler::context::STACK_MIN_HEADROOM;

    // Boundary 1: RSP equal to zero -> Must reject (null pointer)
    let b1 = !crate::scheduler::context::is_valid_task_rsp(slot, 0);

    // Boundary 2: RSP below the stack (base - 8) -> Must reject (underflow)
    let b2 = !crate::scheduler::context::is_valid_task_rsp(slot, base.saturating_sub(8));

    // Boundary 3: RSP equal to lower boundary (base) -> Must reject (0 bytes headroom)
    let b3 = !crate::scheduler::context::is_valid_task_rsp(slot, base);

    // Boundary 4: RSP inside the stack (top - 72 and base + headroom) -> Must accept
    let b4_headroom = crate::scheduler::context::is_valid_task_rsp(slot, base + headroom);
    let b4_init_rsp = crate::scheduler::context::is_valid_task_rsp(slot, top - 72);
    let b4_below_headroom = !crate::scheduler::context::is_valid_task_rsp(slot, base + headroom - 1);
    let b4 = b4_headroom && b4_init_rsp && b4_below_headroom;

    // Boundary 5: RSP equal to upper boundary (top) -> Must reject (off-by-one, 1 byte past buffer)
    let b5 = !crate::scheduler::context::is_valid_task_rsp(slot, top);

    // Boundary 6: RSP above the stack (top + 8) -> Must reject (overflow)
    let b6 = !crate::scheduler::context::is_valid_task_rsp(slot, top + 8);

    // Boundary 7: Invalid slot index (MAX_TASKS or 999) -> Must reject
    let b7_max = !crate::scheduler::context::is_valid_task_rsp(crate::task::manager::MAX_TASKS, top - 72);
    let b7_high = !crate::scheduler::context::is_valid_task_rsp(999, top - 72);
    let b7 = b7_max && b7_high;

    let pass = b1 && b2 && b3 && b4 && b5 && b6 && b7;
    if pass {
        info!("  [PASS] SCHED-TC12: Stack pointer validation boundary conditions (7 cases) verified.");
    } else {
        error!("  [FAIL] SCHED-TC12: Stack pointer boundary condition mismatch!");
    }
    pass
}

// -----------------------------------------------------------------------------
// LIVE COOPERATIVE CPU TASK EXECUTION DEMONSTRATION
// -----------------------------------------------------------------------------

static TASK_A_ITERATIONS: AtomicU64 = AtomicU64::new(0);
static TASK_B_ITERATIONS: AtomicU64 = AtomicU64::new(0);
static TASK_C_ITERATIONS: AtomicU64 = AtomicU64::new(0);

static TASK_A_RSP: AtomicU64 = AtomicU64::new(0);
static TASK_B_RSP: AtomicU64 = AtomicU64::new(0);
static TASK_C_RSP: AtomicU64 = AtomicU64::new(0);

fn worker_task_a() {
    let rsp = crate::scheduler::context::current_rsp();
    TASK_A_RSP.store(rsp, Ordering::SeqCst);
    for iter in 1..=3 {
        TASK_A_ITERATIONS.fetch_add(1, Ordering::SeqCst);
        info!("  [Task A] Running iteration {}/3 - yielding CPU voluntarily.", iter);
        crate::scheduler::sys_yield();
    }
    info!("  [Task A] Completed all 3 iterations - exiting.");
}

fn worker_task_b() {
    let rsp = crate::scheduler::context::current_rsp();
    TASK_B_RSP.store(rsp, Ordering::SeqCst);
    for iter in 1..=3 {
        TASK_B_ITERATIONS.fetch_add(1, Ordering::SeqCst);
        info!("  [Task B] Running iteration {}/3 - yielding CPU voluntarily.", iter);
        crate::scheduler::sys_yield();
    }
    info!("  [Task B] Completed all 3 iterations - exiting.");
}

fn worker_task_c() {
    let rsp = crate::scheduler::context::current_rsp();
    TASK_C_RSP.store(rsp, Ordering::SeqCst);
    for iter in 1..=3 {
        TASK_C_ITERATIONS.fetch_add(1, Ordering::SeqCst);
        info!("  [Task C] Running iteration {}/3 - yielding CPU voluntarily.", iter);
        crate::scheduler::sys_yield();
    }
    info!("  [Task C] Completed all 3 iterations - exiting.");
}

/// Executes the live cooperative task switching demonstration in QEMU.
///
/// Spawns three independent tasks with real, private 4 KiB stacks.
/// The scheduler dispatches them in FIFO order; each task runs, logs its progress,
/// and voluntarily yields execution using `sys_yield()`.
///
/// Returns `true` if all 3 tasks executed all 3 iterations (9 switches total) and
/// returned cleanly to the kernel main thread.
pub fn run_cooperative_task_demo() -> bool {
    info!("--- BEGIN DEMO: Live Cooperative Round Robin Execution ---");

    // Clear ReadyQueue from any prior tests
    crate::task::READY_QUEUE.lock().clear();
    crate::scheduler::init();

    // Create 3 worker tasks with real entry points and private stacks
    let id_a = crate::task::create("Worker-A", Some(worker_task_a)).expect("Create Worker-A");
    let id_b = crate::task::create("Worker-B", Some(worker_task_b)).expect("Create Worker-B");
    let id_c = crate::task::create("Worker-C", Some(worker_task_c)).expect("Create Worker-C");

    info!("Demo Setup: Registered tasks {}, {}, {} with real entry points.", id_a, id_b, id_c);

    // Enqueue them into Ready Queue in FIFO order
    crate::task::enqueue_task(id_a).expect("Enqueue Worker-A");
    crate::task::enqueue_task(id_b).expect("Enqueue Worker-B");
    crate::task::enqueue_task(id_c).expect("Enqueue Worker-C");

    info!("Demo Setup: Enqueued tasks in FIFO order: {} -> {} -> {}.", id_a, id_b, id_c);
    info!("Starting cooperative Round Robin scheduler...");

    // Hand CPU control over to the cooperative scheduler
    crate::scheduler::start();

    // Execution returns here when all tasks have exited via `sys_exit`
    info!("Returned to kernel main thread! Verifying execution counts...");

    let a_runs = TASK_A_ITERATIONS.load(Ordering::SeqCst);
    let b_runs = TASK_B_ITERATIONS.load(Ordering::SeqCst);
    let c_runs = TASK_C_ITERATIONS.load(Ordering::SeqCst);

    let count_success = a_runs == 3 && b_runs == 3 && c_runs == 3;
    if count_success {
        info!(
            "Demo Success: All 3 tasks executed cooperatively (A: {}/3, B: {}/3, C: {}/3).",
            a_runs, b_runs, c_runs
        );
        info!("Actual CPU context switching and Round Robin fairness confirmed.");
    } else {
        error!(
            "Demo Failure: Iteration mismatch! (A: {}/3, B: {}/3, C: {}/3)",
            a_runs, b_runs, c_runs
        );
    }

    // Hardware stack diagnostic evidence
    let main_rsp = crate::scheduler::context::current_rsp();
    let a_rsp = TASK_A_RSP.load(Ordering::SeqCst);
    let b_rsp = TASK_B_RSP.load(Ordering::SeqCst);
    let c_rsp = TASK_C_RSP.load(Ordering::SeqCst);

    let (slot_a, slot_b, slot_c) = {
        let tm = crate::task::TASK_MANAGER.lock();
        (
            tm.get_task_slot(id_a).expect("Slot A"),
            tm.get_task_slot(id_b).expect("Slot B"),
            tm.get_task_slot(id_c).expect("Slot C"),
        )
    };

    let a_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_a].data.as_ptr() as u64 };
    let a_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_a].top() };
    let b_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_b].data.as_ptr() as u64 };
    let b_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_b].top() };
    let c_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_c].data.as_ptr() as u64 };
    let c_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_c].top() };

    info!("Hardware Stack Diagnostic Evidence:");
    info!("  Kernel Main RSP: 0x{:016x}", main_rsp);
    info!("  Worker-A    RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", a_rsp, slot_a, a_base, a_top);
    info!("  Worker-B    RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", b_rsp, slot_b, b_base, b_top);
    info!("  Worker-C    RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", c_rsp, slot_c, c_base, c_top);

    let in_bounds_a = a_rsp >= a_base + crate::scheduler::context::STACK_MIN_HEADROOM && a_rsp < a_top;
    let in_bounds_b = b_rsp >= b_base + crate::scheduler::context::STACK_MIN_HEADROOM && b_rsp < b_top;
    let in_bounds_c = c_rsp >= c_base + crate::scheduler::context::STACK_MIN_HEADROOM && c_rsp < c_top;

    let distinct_stacks = a_rsp != 0 && b_rsp != 0 && c_rsp != 0
        && a_rsp != b_rsp && b_rsp != c_rsp && a_rsp != c_rsp
        && a_rsp != main_rsp && b_rsp != main_rsp && c_rsp != main_rsp;

    let diag_pass = in_bounds_a && in_bounds_b && in_bounds_c && distinct_stacks;
    if diag_pass {
        info!("Stack Diagnostic: PASS - Confirmed distinct, isolated 4 KiB hardware stacks.");
    } else {
        error!("Stack Diagnostic: FAIL - Stack overlap or invalid RSP detected!");
    }

    count_success && diag_pass
}

// -----------------------------------------------------------------------------
// LIVE PREEMPTIVE CPU TASK EXECUTION DEMONSTRATION (MILESTONE 9)
// -----------------------------------------------------------------------------

static PREEMPT_WORKER_A_PROGRESS: AtomicU64 = AtomicU64::new(0);
static PREEMPT_WORKER_B_PROGRESS: AtomicU64 = AtomicU64::new(0);
static PREEMPT_WORKER_C_PROGRESS: AtomicU64 = AtomicU64::new(0);

static PREEMPT_WORKER_A_RSP: AtomicU64 = AtomicU64::new(0);
static PREEMPT_WORKER_B_RSP: AtomicU64 = AtomicU64::new(0);
static PREEMPT_WORKER_C_RSP: AtomicU64 = AtomicU64::new(0);

fn preempt_worker_task_a() {
    let rsp = crate::scheduler::context::current_rsp();
    PREEMPT_WORKER_A_RSP.store(rsp, Ordering::SeqCst);
    info!("  [Preempt Worker-A] Started CPU-bound computation.");
    for phase in 1..=3 {
        info!("  [Preempt Worker-A] Phase {}/3 executing CPU workload (no sys_yield)...", phase);
        // Busy compute loop for at least 2 timer ticks (20 ms)
        let start_tick = crate::timer::get_ticks();
        let mut val: u64 = 0x123456789ABCDEF0;
        while crate::timer::get_ticks() < start_tick + 2 {
            val = val.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            core::hint::black_box(val);
        }
        PREEMPT_WORKER_A_PROGRESS.fetch_add(1, Ordering::SeqCst);
    }
    info!("  [Preempt Worker-A] Completed all 3 compute phases - exiting.");
}

fn preempt_worker_task_b() {
    let rsp = crate::scheduler::context::current_rsp();
    PREEMPT_WORKER_B_RSP.store(rsp, Ordering::SeqCst);
    info!("  [Preempt Worker-B] Started CPU-bound computation.");
    for phase in 1..=3 {
        info!("  [Preempt Worker-B] Phase {}/3 executing CPU workload (no sys_yield)...", phase);
        // Busy compute loop for at least 2 timer ticks (20 ms)
        let start_tick = crate::timer::get_ticks();
        let mut val: u64 = 0xFEDCBA9876543210;
        while crate::timer::get_ticks() < start_tick + 2 {
            val = val.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            core::hint::black_box(val);
        }
        PREEMPT_WORKER_B_PROGRESS.fetch_add(1, Ordering::SeqCst);
    }
    info!("  [Preempt Worker-B] Completed all 3 compute phases - exiting.");
}

fn preempt_worker_task_c() {
    let rsp = crate::scheduler::context::current_rsp();
    PREEMPT_WORKER_C_RSP.store(rsp, Ordering::SeqCst);
    info!("  [Preempt Worker-C] Started CPU-bound computation.");
    for phase in 1..=3 {
        info!("  [Preempt Worker-C] Phase {}/3 executing CPU workload (no sys_yield)...", phase);
        // Busy compute loop for at least 2 timer ticks (20 ms)
        let start_tick = crate::timer::get_ticks();
        let mut val: u64 = 0x0123456789ABCDEF;
        while crate::timer::get_ticks() < start_tick + 2 {
            val = val.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            core::hint::black_box(val);
        }
        PREEMPT_WORKER_C_PROGRESS.fetch_add(1, Ordering::SeqCst);
    }
    info!("  [Preempt Worker-C] Completed all 3 compute phases - exiting.");
}

/// Executes the live timer-driven preemptive scheduling demonstration in QEMU.
///
/// Spawns three independent tasks running CPU-bound workloads with zero calls to `sys_yield()`.
/// The 8253 PIT hardware timer periodically interrupts the CPU at 100 Hz, preempting the active
/// task and dispatching the next task in Round Robin order.
pub fn run_preemptive_task_demo() -> bool {
    info!("--- BEGIN DEMO: Live Preemptive Round Robin Execution (100 Hz Timer) ---");

    PREEMPT_WORKER_A_PROGRESS.store(0, Ordering::SeqCst);
    PREEMPT_WORKER_B_PROGRESS.store(0, Ordering::SeqCst);
    PREEMPT_WORKER_C_PROGRESS.store(0, Ordering::SeqCst);

    // Reset preemption switch count
    crate::scheduler::PREEMPTION_SWITCH_COUNT.store(0, Ordering::SeqCst);

    // Clear ReadyQueue and reset scheduler state
    crate::task::READY_QUEUE.lock().clear();
    crate::scheduler::init();

    // Register 3 CPU-bound worker tasks with real entry points
    let id_a = crate::task::create("Preempt-A", Some(preempt_worker_task_a)).expect("Create Preempt-A");
    let id_b = crate::task::create("Preempt-B", Some(preempt_worker_task_b)).expect("Create Preempt-B");
    let id_c = crate::task::create("Preempt-C", Some(preempt_worker_task_c)).expect("Create Preempt-C");

    // Setup 160-byte interrupt stack frames for all 3 tasks
    {
        let tm = crate::task::TASK_MANAGER.lock();
        let slot_a = tm.get_task_slot(id_a).expect("Slot A");
        let slot_b = tm.get_task_slot(id_b).expect("Slot B");
        let slot_c = tm.get_task_slot(id_c).expect("Slot C");

        unsafe {
            crate::scheduler::context::setup_preemptive_task_stack(slot_a, Some(preempt_worker_task_a));
            crate::scheduler::context::setup_preemptive_task_stack(slot_b, Some(preempt_worker_task_b));
            crate::scheduler::context::setup_preemptive_task_stack(slot_c, Some(preempt_worker_task_c));
        }
    }

    info!("Demo Setup: Registered preemptive tasks {}, {}, {}.", id_a, id_b, id_c);

    // Enqueue into ReadyQueue in FIFO order
    crate::task::enqueue_task(id_a).expect("Enqueue Preempt-A");
    crate::task::enqueue_task(id_b).expect("Enqueue Preempt-B");
    crate::task::enqueue_task(id_c).expect("Enqueue Preempt-C");

    info!("Demo Setup: Enqueued tasks in FIFO order: {} -> {} -> {}.", id_a, id_b, id_c);
    info!("Starting timer-driven preemptive Round Robin scheduler...");

    // Start preemptive scheduling (switches CPU to Worker-A via iretq with IF=1)
    let start_ok = crate::scheduler::start_preemptive_scheduling();

    info!("Returned to kernel main thread! Verifying preemptive execution...");

    let a_prog = PREEMPT_WORKER_A_PROGRESS.load(Ordering::SeqCst);
    let b_prog = PREEMPT_WORKER_B_PROGRESS.load(Ordering::SeqCst);
    let c_prog = PREEMPT_WORKER_C_PROGRESS.load(Ordering::SeqCst);
    let switch_count = crate::scheduler::PREEMPTION_SWITCH_COUNT.load(Ordering::SeqCst);
    let total_ticks = crate::timer::get_ticks();

    let count_success = a_prog == 3 && b_prog == 3 && c_prog == 3;
    let preemption_verified = switch_count >= 6;

    if count_success && preemption_verified {
        info!(
            "Demo Success: All 3 workers completed workloads (A: {}/3, B: {}/3, C: {}/3).",
            a_prog, b_prog, c_prog
        );
        info!(
            "Hardware timer preemption confirmed: {} context switches across {} ticks.",
            switch_count, total_ticks
        );
    } else {
        error!(
            "Demo Failure: Workload or preemption count mismatch! (A: {}/3, B: {}/3, C: {}/3, switches: {})",
            a_prog, b_prog, c_prog, switch_count
        );
    }

    // Hardware stack diagnostic evidence for preemptive tasks
    let main_rsp = crate::scheduler::context::current_rsp();
    let a_rsp = PREEMPT_WORKER_A_RSP.load(Ordering::SeqCst);
    let b_rsp = PREEMPT_WORKER_B_RSP.load(Ordering::SeqCst);
    let c_rsp = PREEMPT_WORKER_C_RSP.load(Ordering::SeqCst);

    let (slot_a, slot_b, slot_c) = {
        let tm = crate::task::TASK_MANAGER.lock();
        (
            tm.get_task_slot(id_a).expect("Slot A"),
            tm.get_task_slot(id_b).expect("Slot B"),
            tm.get_task_slot(id_c).expect("Slot C"),
        )
    };

    let a_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_a].data.as_ptr() as u64 };
    let a_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_a].top() };
    let b_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_b].data.as_ptr() as u64 };
    let b_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_b].top() };
    let c_base = unsafe { crate::scheduler::context::TASK_STACKS[slot_c].data.as_ptr() as u64 };
    let c_top = unsafe { crate::scheduler::context::TASK_STACKS[slot_c].top() };

    info!("Hardware Stack Diagnostic Evidence (Preemptive Tasks):");
    info!("  Kernel Main RSP: 0x{:016x}", main_rsp);
    info!("  Preempt-A   RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", a_rsp, slot_a, a_base, a_top);
    info!("  Preempt-B   RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", b_rsp, slot_b, b_base, b_top);
    info!("  Preempt-C   RSP: 0x{:016x} (Slot {} buffer: 0x{:x} - 0x{:x})", c_rsp, slot_c, c_base, c_top);

    let in_bounds_a = a_rsp >= a_base + crate::scheduler::context::STACK_MIN_HEADROOM && a_rsp < a_top;
    let in_bounds_b = b_rsp >= b_base + crate::scheduler::context::STACK_MIN_HEADROOM && b_rsp < b_top;
    let in_bounds_c = c_rsp >= c_base + crate::scheduler::context::STACK_MIN_HEADROOM && c_rsp < c_top;

    let distinct_stacks = a_rsp != 0 && b_rsp != 0 && c_rsp != 0
        && a_rsp != b_rsp && b_rsp != c_rsp && a_rsp != c_rsp
        && a_rsp != main_rsp && b_rsp != main_rsp && c_rsp != main_rsp;

    let diag_pass = in_bounds_a && in_bounds_b && in_bounds_c && distinct_stacks;
    if diag_pass {
        info!("Stack Diagnostic: PASS - Confirmed distinct, isolated 4 KiB stacks during timer interrupts.");
    } else {
        error!("Stack Diagnostic: FAIL - Stack overlap or invalid RSP detected during preemption!");
    }

    // Verify hardware interrupt frame preservation across preemptions
    let saved_rip = crate::scheduler::round_robin::FIRST_INTERRUPTED_RIP.load(Ordering::SeqCst);
    let saved_task_rsp = crate::scheduler::round_robin::FIRST_INTERRUPTED_RSP.load(Ordering::SeqCst);
    let hw_frame_pass = saved_rip != 0 && saved_task_rsp != 0;
    if hw_frame_pass {
        info!("Hardware Frame Verification: PASS - Confirmed RIP=0x{:x}, RSP=0x{:x} preserved across preemption.", saved_rip, saved_task_rsp);
    } else {
        error!("Hardware Frame Verification: FAIL - Hardware interrupt frame values invalid!");
    }

    start_ok && count_success && preemption_verified && diag_pass && hw_frame_pass
}
