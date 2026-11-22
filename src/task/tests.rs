//! In-kernel automated verification tests for Milestone 6: Task Representation and Task States.
//!
//! Executes programmatic assertions verifying task creation, state transitions,
//! invalid transition rejection, termination, and task manager lookups.

use crate::{error, info};
use crate::task::manager::{TaskManager, TaskManagerError, MAX_TASKS};
use crate::task::ready_queue::{QueueError, ReadyQueue, QUEUE_CAPACITY};
use crate::task::state::{StateTransitionError, TaskState};
use crate::task::task::{Priority, Task, TaskId};

/// Entry point to execute all Milestone 6 kernel verification tests.
///
/// Returns true if all assertions succeed, false otherwise.
pub fn run_task_tests() -> bool {
    info!("========================================================");
    info!("    RUNNING MILESTONE 6: TASK MANAGEMENT TEST SUITE     ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_task_creation_and_initial_state();
    all_passed &= test_unique_task_ids();
    all_passed &= test_valid_state_transitions();
    all_passed &= test_invalid_state_transitions_rejected();
    all_passed &= test_task_termination();
    all_passed &= test_task_manager_registration_and_lookup();
    all_passed &= test_task_manager_capacity_limit();

    if all_passed {
        info!(">>> MILESTONE 6 KERNEL TESTS: ALL 7 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 6 KERNEL TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// Test 1: Task creation and initial state verification.
fn test_task_creation_and_initial_state() -> bool {
    let task = Task::new("Worker-Init", None);

    let name_ok = task.name == "Worker-Init";
    let state_ok = task.state == TaskState::Ready;
    let prio_ok = task.priority == Priority::Normal;
    let runnable_ok = task.is_ready() && !task.is_terminated();
    let ticks_ok = task.ticks == 0;

    let passed = name_ok && state_ok && prio_ok && runnable_ok && ticks_ok;
    if passed {
        info!("  [PASS] TC1: Task creation & initial READY state verified (ID: {}).", task.id);
    } else {
        error!("  [FAIL] TC1: Task creation initial state assertion failed!");
    }
    passed
}

/// Test 2: Unique sequential Task ID assignment.
fn test_unique_task_ids() -> bool {
    let t1 = Task::new("T-ID-1", None);
    let t2 = Task::new("T-ID-2", None);
    let t3 = Task::new("T-ID-3", None);
    let t4 = Task::new("T-ID-4", None);
    let t5 = Task::new("T-ID-5", None);

    let unique_ok = t1.id != t2.id
        && t2.id != t3.id
        && t3.id != t4.id
        && t4.id != t5.id;

    let sequential_ok = t2.id.as_u64() > t1.id.as_u64()
        && t3.id.as_u64() > t2.id.as_u64()
        && t4.id.as_u64() > t3.id.as_u64()
        && t5.id.as_u64() > t4.id.as_u64();

    let passed = unique_ok && sequential_ok;
    if passed {
        info!(
            "  [PASS] TC2: Unique sequential Task IDs verified ({} -> {} -> {}).",
            t1.id, t2.id, t5.id
        );
    } else {
        error!("  [FAIL] TC2: Unique Task ID assertion failed!");
    }
    passed
}

/// Test 3: Valid life-cycle state transitions.
fn test_valid_state_transitions() -> bool {
    let mut task = Task::new("Lifecycle-Task", None);

    // Initial state: READY
    if task.state != TaskState::Ready {
        error!("  [FAIL] TC3: Expected initial READY state.");
        return false;
    }

    // READY -> RUNNING (Scheduled)
    if task.transition_to(TaskState::Running).is_err() || task.state != TaskState::Running {
        error!("  [FAIL] TC3: Transition READY -> RUNNING failed.");
        return false;
    }

    // RUNNING -> BLOCKED (Waiting on I/O or lock)
    if task.transition_to(TaskState::Blocked).is_err() || task.state != TaskState::Blocked {
        error!("  [FAIL] TC3: Transition RUNNING -> BLOCKED failed.");
        return false;
    }

    // BLOCKED -> READY (Resource available)
    if task.transition_to(TaskState::Ready).is_err() || task.state != TaskState::Ready {
        error!("  [FAIL] TC3: Transition BLOCKED -> READY failed.");
        return false;
    }

    // READY -> RUNNING (Dispatched again)
    if task.transition_to(TaskState::Running).is_err() || task.state != TaskState::Running {
        error!("  [FAIL] TC3: Second transition READY -> RUNNING failed.");
        return false;
    }

    // RUNNING -> READY (Preemption / Yield)
    if task.transition_to(TaskState::Ready).is_err() || task.state != TaskState::Ready {
        error!("  [FAIL] TC3: Transition RUNNING -> READY (Preemption) failed.");
        return false;
    }

    // READY -> TERMINATED (Cancelled)
    if task.transition_to(TaskState::Terminated).is_err() || task.state != TaskState::Terminated {
        error!("  [FAIL] TC3: Transition READY -> TERMINATED failed.");
        return false;
    }

    info!("  [PASS] TC3: Valid state transitions (Ready->Running->Blocked->Ready->Terminated) verified.");
    true
}

/// Test 4: Rejection of illegal state transitions.
fn test_invalid_state_transitions_rejected() -> bool {
    // Subtest A: Terminated -> Running must fail
    let mut t_term = Task::new("Term-Task", None);
    let _ = t_term.terminate();
    let err_term_to_run = t_term.transition_to(TaskState::Running);
    let a_ok = err_term_to_run == Err(StateTransitionError {
        from: TaskState::Terminated,
        to: TaskState::Running,
    });

    // Subtest B: Terminated -> Ready must fail
    let err_term_to_ready = t_term.transition_to(TaskState::Ready);
    let b_ok = err_term_to_ready == Err(StateTransitionError {
        from: TaskState::Terminated,
        to: TaskState::Ready,
    });

    // Subtest C: Blocked -> Running directly must fail (must go through Ready)
    let mut t_block = Task::new("Block-Task", None);
    let _ = t_block.transition_to(TaskState::Running);
    let _ = t_block.transition_to(TaskState::Blocked);
    let err_block_to_run = t_block.transition_to(TaskState::Running);
    let c_ok = err_block_to_run == Err(StateTransitionError {
        from: TaskState::Blocked,
        to: TaskState::Running,
    });

    // Subtest D: Ready -> Blocked directly must fail (must go through Running)
    let mut t_ready = Task::new("Ready-Task", None);
    let err_ready_to_block = t_ready.transition_to(TaskState::Blocked);
    let d_ok = err_ready_to_block == Err(StateTransitionError {
        from: TaskState::Ready,
        to: TaskState::Blocked,
    });

    let passed = a_ok && b_ok && c_ok && d_ok;
    if passed {
        info!("  [PASS] TC4: Illegal transitions (Terminated->*, Blocked->Running, Ready->Blocked) correctly rejected.");
    } else {
        error!(
            "  [FAIL] TC4: Invalid transition rejection assertion failed: A={}, B={}, C={}, D={}",
            a_ok, b_ok, c_ok, d_ok
        );
    }
    passed
}

/// Test 5: Task termination and finality.
fn test_task_termination() -> bool {
    let mut task = Task::new("To-Terminate", None);
    let _ = task.transition_to(TaskState::Running);

    let term_res = task.terminate();
    let is_term = task.is_terminated();
    let not_ready = !task.is_ready();

    // Verify that subsequent transition from Terminated is rejected
    let post_term_res = task.transition_to(TaskState::Ready);

    let passed = term_res.is_ok() && is_term && not_ready && post_term_res.is_err();
    if passed {
        info!("  [PASS] TC5: Task termination and terminal state permanence verified.");
    } else {
        error!("  [FAIL] TC5: Task termination assertion failed!");
    }
    passed
}

/// Test 6: TaskManager registration, lookup, and state tracking.
fn test_task_manager_registration_and_lookup() -> bool {
    let mut manager = TaskManager::new();

    // Register 3 tasks
    let id_a = manager.create_task("Alpha", None);
    let id_b = manager.create_task_with_priority("Beta", Priority::High, None);
    let id_c = manager.create_task_with_priority("Gamma", Priority::Low, None);

    if id_a.is_err() || id_b.is_err() || id_c.is_err() {
        error!("  [FAIL] TC6: Failed to create tasks in TaskManager.");
        return false;
    }

    let (id_a, id_b, id_c) = (id_a.unwrap(), id_b.unwrap(), id_c.unwrap());

    // Verify counts
    let count_ok = manager.count() == 3 && manager.active_count() == 3;

    // Verify lookup by TaskId
    let task_a = manager.get_task(id_a);
    let lookup_a_ok = task_a.map_or(false, |t| t.name == "Alpha" && t.state == TaskState::Ready);

    let task_b = manager.get_task(id_b);
    let lookup_b_ok = task_b.map_or(false, |t| t.priority == Priority::High);

    let task_c = manager.get_task(id_c);
    let lookup_c_ok = task_c.map_or(false, |t| t.priority == Priority::Low && t.name == "Gamma");

    // Transition Beta to Running, then Terminated
    let trans_ok = manager.transition_task(id_b, TaskState::Running).is_ok();
    let term_ok = manager.terminate_task(id_b).is_ok();

    // Active count should now be 2, total count 3
    let active_ok = manager.active_count() == 2 && manager.count() == 3;

    // Lookup non-existent ID
    let missing_ok = manager.get_task(TaskId(999999)).is_none();

    let passed = count_ok && lookup_a_ok && lookup_b_ok && lookup_c_ok && trans_ok && term_ok && active_ok && missing_ok;
    if passed {
        info!(
            "  [PASS] TC6: TaskManager registration, lookup, and active counting verified (Active: {}/{}).",
            manager.active_count(),
            manager.count()
        );
    } else {
        error!("  [FAIL] TC6: TaskManager lookup and tracking assertion failed!");
    }
    passed
}

/// Test 7: TaskManager capacity limits and boundary enforcement.
fn test_task_manager_capacity_limit() -> bool {
    let mut manager = TaskManager::new();

    // Fill all MAX_TASKS slots
    for i in 0..MAX_TASKS {
        if manager.create_task("BatchTask", None).is_err() {
            error!("  [FAIL] TC7: Failed to fill slot {} in TaskManager.", i);
            return false;
        }
    }

    // Slot 33 should fail with TableFull error
    let overflow_res = manager.create_task("OverflowTask", None);
    let overflow_ok = overflow_res == Err(TaskManagerError::TableFull);

    if overflow_ok {
        info!("  [PASS] TC7: TaskManager capacity limit ({} tasks) safely enforced.", MAX_TASKS);
    } else {
        error!("  [FAIL] TC7: TaskManager capacity overflow was not rejected!");
    }
    overflow_ok
}

/// Entry point to execute all Milestone 7 Ready Queue verification tests.
pub fn run_ready_queue_tests() -> bool {
    info!("========================================================");
    info!("     RUNNING MILESTONE 7: READY QUEUE TEST SUITE        ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_empty_queue_behavior();
    all_passed &= test_enqueue_single_and_multiple_tasks();
    all_passed &= test_fifo_dequeue_order();
    all_passed &= test_peek_does_not_remove();
    all_passed &= test_duplicate_task_rejected();
    all_passed &= test_full_queue_capacity_32();
    all_passed &= test_dequeue_from_empty_queue();
    all_passed &= test_remove_specific_task();
    all_passed &= test_circular_wraparound_consistency();
    all_passed &= test_only_ready_tasks_accepted();
    all_passed &= test_blocked_task_reenqueued_after_ready();
    all_passed &= test_queue_len_tracking_across_interleaved_ops();

    if all_passed {
        info!(">>> MILESTONE 7 KERNEL TESTS: ALL 12 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 7 KERNEL TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// RQ-Test 1: Empty queue initial state and underflow behavior.
fn test_empty_queue_behavior() -> bool {
    let mut q = ReadyQueue::new();

    let empty_ok = q.is_empty();
    let not_full_ok = !q.is_full();
    let len_ok = q.len() == 0;
    let peek_none = q.peek().is_none();
    let dequeue_none = q.dequeue().is_none();

    let passed = empty_ok && not_full_ok && len_ok && peek_none && dequeue_none;
    if passed {
        info!("  [PASS] RQ-TC1: Empty queue behavior (len=0, is_empty=true, dequeue=None) verified.");
    } else {
        error!("  [FAIL] RQ-TC1: Empty queue behavior assertion failed!");
    }
    passed
}

/// RQ-Test 2: Enqueue single and multiple tasks.
fn test_enqueue_single_and_multiple_tasks() -> bool {
    let mut q = ReadyQueue::new();

    let enq1 = q.enqueue_raw(TaskId(101));
    let len1 = q.len() == 1 && !q.is_empty();

    let enq2 = q.enqueue_raw(TaskId(102));
    let enq3 = q.enqueue_raw(TaskId(103));
    let len3 = q.len() == 3;

    let passed = enq1.is_ok() && len1 && enq2.is_ok() && enq3.is_ok() && len3;
    if passed {
        info!("  [PASS] RQ-TC2: Enqueue single and multiple tasks (count: 3) verified.");
    } else {
        error!("  [FAIL] RQ-TC2: Enqueue assertion failed!");
    }
    passed
}

/// RQ-Test 3: FIFO (First In, First Out) dequeue order preservation.
fn test_fifo_dequeue_order() -> bool {
    let mut q = ReadyQueue::new();

    let _ = q.enqueue_raw(TaskId(10));
    let _ = q.enqueue_raw(TaskId(20));
    let _ = q.enqueue_raw(TaskId(30));

    let d1 = q.dequeue();
    let d2 = q.dequeue();
    let d3 = q.dequeue();
    let d4 = q.dequeue();

    let order_ok = d1 == Some(TaskId(10))
        && d2 == Some(TaskId(20))
        && d3 == Some(TaskId(30))
        && d4 == None;

    let empty_after = q.is_empty() && q.len() == 0;

    let passed = order_ok && empty_after;
    if passed {
        info!("  [PASS] RQ-TC3: Strict FIFO order (T10 -> T20 -> T30 -> None) verified.");
    } else {
        error!("  [FAIL] RQ-TC3: FIFO order assertion failed!");
    }
    passed
}

/// RQ-Test 4: Peek does not remove elements from the queue.
fn test_peek_does_not_remove() -> bool {
    let mut q = ReadyQueue::new();
    let _ = q.enqueue_raw(TaskId(50));
    let _ = q.enqueue_raw(TaskId(60));

    let p1 = q.peek();
    let len_after_p1 = q.len();
    let p2 = q.peek();
    let len_after_p2 = q.len();

    let d1 = q.dequeue();

    let passed = p1 == Some(TaskId(50))
        && len_after_p1 == 2
        && p2 == Some(TaskId(50))
        && len_after_p2 == 2
        && d1 == Some(TaskId(50));

    if passed {
        info!("  [PASS] RQ-TC4: Peek inspection without removal verified.");
    } else {
        error!("  [FAIL] RQ-TC4: Peek assertion failed!");
    }
    passed
}

/// RQ-Test 5: Duplicate task insertion rejection.
fn test_duplicate_task_rejected() -> bool {
    let mut q = ReadyQueue::new();
    let _ = q.enqueue_raw(TaskId(77));

    let dup_res = q.enqueue_raw(TaskId(77));
    let dup_rejected = dup_res == Err(QueueError::DuplicateTask(TaskId(77)));
    let len_unaffected = q.len() == 1;

    let passed = dup_rejected && len_unaffected;
    if passed {
        info!("  [PASS] RQ-TC5: Duplicate task insertion cleanly rejected.");
    } else {
        error!("  [FAIL] RQ-TC5: Duplicate task assertion failed!");
    }
    passed
}

/// RQ-Test 6: Full queue behavior at capacity 32.
fn test_full_queue_capacity_32() -> bool {
    let mut q = ReadyQueue::new();

    for i in 1..=QUEUE_CAPACITY {
        let res = q.enqueue_raw(TaskId(i as u64));
        if res.is_err() {
            error!("  [FAIL] RQ-TC6: Failed to enqueue item {} of {}", i, QUEUE_CAPACITY);
            return false;
        }
    }

    let is_full = q.is_full();
    let len_32 = q.len() == QUEUE_CAPACITY;

    // 33rd enqueue must return QueueFull
    let overflow_res = q.enqueue_raw(TaskId(999));
    let overflow_rejected = overflow_res == Err(QueueError::QueueFull);
    let len_still_32 = q.len() == QUEUE_CAPACITY;

    let passed = is_full && len_32 && overflow_rejected && len_still_32;
    if passed {
        info!("  [PASS] RQ-TC6: Full queue capacity ({} entries) & overflow rejection verified.", QUEUE_CAPACITY);
    } else {
        error!("  [FAIL] RQ-TC6: Capacity 32 overflow assertion failed!");
    }
    passed
}

/// RQ-Test 7: Dequeue underflow safety.
fn test_dequeue_from_empty_queue() -> bool {
    let mut q = ReadyQueue::new();

    let d1 = q.dequeue();
    let d2 = q.dequeue();
    let len_zero = q.len() == 0;

    let passed = d1 == None && d2 == None && len_zero;
    if passed {
        info!("  [PASS] RQ-TC7: Dequeue from empty queue safely returns None without panic.");
    } else {
        error!("  [FAIL] RQ-TC7: Dequeue underflow assertion failed!");
    }
    passed
}

/// RQ-Test 8: Arbitrary task removal from queue while preserving order.
fn test_remove_specific_task() -> bool {
    let mut q = ReadyQueue::new();
    let _ = q.enqueue_raw(TaskId(1));
    let _ = q.enqueue_raw(TaskId(2));
    let _ = q.enqueue_raw(TaskId(3));
    let _ = q.enqueue_raw(TaskId(4));

    // Remove middle task T3
    let rem_t3 = q.remove(TaskId(3));
    let contains_t3 = q.contains(TaskId(3));
    let len_3 = q.len() == 3;

    // Remove non-existent task
    let rem_nonexistent = q.remove(TaskId(999));

    // Verify remaining dequeue order: T1, T2, T4
    let d1 = q.dequeue();
    let d2 = q.dequeue();
    let d3 = q.dequeue();
    let d4 = q.dequeue();

    let order_ok = d1 == Some(TaskId(1))
        && d2 == Some(TaskId(2))
        && d3 == Some(TaskId(4))
        && d4 == None;

    let passed = rem_t3 && !contains_t3 && len_3 && !rem_nonexistent && order_ok;
    if passed {
        info!("  [PASS] RQ-TC8: Removal of specific task (T3) and compaction verified (Order: T1->T2->T4).");
    } else {
        error!("  [FAIL] RQ-TC8: Task removal assertion failed!");
    }
    passed
}

/// RQ-Test 9: Circular buffer wraparound consistency.
fn test_circular_wraparound_consistency() -> bool {
    let mut q = ReadyQueue::new();

    // Enqueue 20 items (T1..=T20)
    for i in 1..=20 {
        let _ = q.enqueue_raw(TaskId(i));
    }

    // Dequeue 15 items (T1..=T15), head advances to index 15
    for i in 1..=15 {
        if q.dequeue() != Some(TaskId(i)) {
            error!("  [FAIL] RQ-TC9: Dequeue mismatch during partial drain");
            return false;
        }
    }
    // Now len=5 (T16..=T20), head=15, tail=20

    // Enqueue 25 items (T21..=T45), tail wraps around past 32 to index 13
    for i in 21..=45 {
        if q.enqueue_raw(TaskId(i)).is_err() {
            error!("  [FAIL] RQ-TC9: Wraparound enqueue failed at item {}", i);
            return false;
        }
    }
    // Total len = 5 + 25 = 30
    let len_30 = q.len() == 30;

    // Dequeue all remaining 30 items in exact sequence (T16..=T45)
    let mut sequence_ok = true;
    for i in 16..=45 {
        if q.dequeue() != Some(TaskId(i)) {
            sequence_ok = false;
            break;
        }
    }
    let empty_end = q.is_empty();

    let passed = len_30 && sequence_ok && empty_end;
    if passed {
        info!("  [PASS] RQ-TC9: Circular buffer index wraparound consistency verified (30 items traversed).");
    } else {
        error!("  [FAIL] RQ-TC9: Circular wraparound assertion failed!");
    }
    passed
}

/// RQ-Test 10: Only tasks in READY state are accepted into the queue.
fn test_only_ready_tasks_accepted() -> bool {
    let mut tm = TaskManager::new();
    let mut q = ReadyQueue::new();

    let id_run = tm.create_task("RunTask", None).unwrap();
    tm.transition_task_sync(id_run, TaskState::Running, &mut q).unwrap();

    let id_block = tm.create_task("BlockTask", None).unwrap();
    tm.transition_task_sync(id_block, TaskState::Running, &mut q).unwrap();
    tm.transition_task_sync(id_block, TaskState::Blocked, &mut q).unwrap();

    let id_term = tm.create_task("TermTask", None).unwrap();
    tm.transition_task_sync(id_term, TaskState::Terminated, &mut q).unwrap();

    let id_ready = tm.create_task("ReadyTask", None).unwrap();

    let res_run = q.enqueue(id_run, &tm);
    let run_rejected = res_run == Err(QueueError::TaskNotReady(id_run, TaskState::Running));

    let res_block = q.enqueue(id_block, &tm);
    let block_rejected = res_block == Err(QueueError::TaskNotReady(id_block, TaskState::Blocked));

    let res_term = q.enqueue(id_term, &tm);
    let term_rejected = res_term == Err(QueueError::TaskNotReady(id_term, TaskState::Terminated));

    let res_ready = q.enqueue(id_ready, &tm);
    let ready_accepted = res_ready.is_ok();

    let len_ok = q.len() == 1;

    let passed = run_rejected && block_rejected && term_rejected && ready_accepted && len_ok;
    if passed {
        info!("  [PASS] RQ-TC10: State filtering (Running/Blocked/Terminated rejected, Ready accepted) verified.");
    } else {
        error!("  [FAIL] RQ-TC10: State filtering assertion failed!");
    }
    passed
}

/// RQ-Test 11: Blocked task re-enqueued after becoming Ready.
fn test_blocked_task_reenqueued_after_ready() -> bool {
    let mut tm = TaskManager::new();
    let mut q = ReadyQueue::new();
    let id = tm.create_task("LifecycleTask", None).unwrap();

    // Phase 1: Task transitions Ready -> Running -> Blocked -> rejected by q.enqueue
    tm.transition_task_sync(id, TaskState::Running, &mut q).unwrap();
    tm.transition_task_sync(id, TaskState::Blocked, &mut q).unwrap();
    let step1_rejected = q.enqueue(id, &tm) == Err(QueueError::TaskNotReady(id, TaskState::Blocked));

    // Phase 2: Resource unblocked -> Task becomes Ready -> auto-sync enqueues it
    tm.transition_task_sync(id, TaskState::Ready, &mut q).unwrap();
    let step2_accepted = q.contains(id) && q.len() == 1;

    // Phase 3: Dequeued for execution
    let step3_dequeued = q.dequeue() == Some(id);

    // Phase 4: Task runs, blocks again, then becomes Ready again -> auto-enqueued successfully
    tm.transition_task_sync(id, TaskState::Running, &mut q).unwrap();
    tm.transition_task_sync(id, TaskState::Blocked, &mut q).unwrap();
    tm.transition_task_sync(id, TaskState::Ready, &mut q).unwrap();
    let step4_reenqueued = q.contains(id) && q.len() == 1;

    let passed = step1_rejected && step2_accepted && step3_dequeued && step4_reenqueued;
    if passed {
        info!("  [PASS] RQ-TC11: Lifecycle re-enqueueing of unblocked tasks verified.");
    } else {
        error!("  [FAIL] RQ-TC11: Re-enqueueing assertion failed!");
    }
    passed
}

/// RQ-Test 12: Interleaved operations len tracking.
fn test_queue_len_tracking_across_interleaved_ops() -> bool {
    let mut q = ReadyQueue::new();

    let mut checks = true;
    checks &= q.len() == 0;

    let _ = q.enqueue_raw(TaskId(1));
    let _ = q.enqueue_raw(TaskId(2));
    checks &= q.len() == 2;

    let _ = q.dequeue();
    checks &= q.len() == 1;

    let _ = q.enqueue_raw(TaskId(3));
    let _ = q.enqueue_raw(TaskId(4));
    checks &= q.len() == 3;

    let _ = q.peek();
    checks &= q.len() == 3;

    let _ = q.remove(TaskId(3));
    checks &= q.len() == 2;

    q.clear();
    checks &= q.len() == 0 && q.is_empty();

    if checks {
        info!("  [PASS] RQ-TC12: Interleaved operations count tracking & clear() verified.");
    } else {
        error!("  [FAIL] RQ-TC12: Interleaved operations assertion failed!");
    }
    checks
}

/// Entry point to execute the Milestone 7 Ready Queue regression test suite.
pub fn run_ready_queue_regression_tests() -> bool {
    info!("========================================================");
    info!("   RUNNING MILESTONE 7: RQ AUTO-SYNC REGRESSION TESTS   ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_reg_running_task_cannot_be_enqueued();
    all_passed &= test_reg_blocked_task_cannot_be_enqueued();
    all_passed &= test_reg_terminated_task_cannot_be_enqueued();
    all_passed &= test_reg_nonexistent_task_id_rejected();
    all_passed &= test_reg_ready_task_can_be_enqueued();
    all_passed &= test_reg_auto_sync_ready_to_running_removes();
    all_passed &= test_reg_auto_sync_lifecycle_and_ready_blocked_rejected();
    all_passed &= test_reg_auto_sync_ready_to_terminated_removes();
    all_passed &= test_reg_auto_sync_blocked_to_ready_appends_tail();
    all_passed &= test_reg_auto_sync_running_to_ready_appends_tail();
    all_passed &= test_reg_auto_sync_repeated_ready_no_duplicate();
    all_passed &= test_reg_auto_sync_failed_transition_preserves_state_and_queue();
    all_passed &= test_reg_auto_sync_queue_full_transition_fails_safely();

    if all_passed {
        info!(">>> MILESTONE 7 REGRESSION TESTS: ALL 13 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 7 REGRESSION TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// Regression Test 1: A Running task cannot be inserted through the public API.
fn test_reg_running_task_cannot_be_enqueued() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("RunTask", None).unwrap();
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();

    let res = rq.enqueue(id, &tm);
    let rejected = res == Err(QueueError::TaskNotReady(id, TaskState::Running));
    let len_zero = rq.len() == 0;

    let passed = rejected && len_zero;
    if passed {
        info!("  [PASS] RQ-REG-1: Running task rejected by public enqueue API (TaskNotReady).");
    } else {
        error!("  [FAIL] RQ-REG-1: Running task was not rejected by public enqueue API!");
    }
    passed
}

/// Regression Test 2: A Blocked task cannot be inserted through the public API.
fn test_reg_blocked_task_cannot_be_enqueued() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("BlockTask", None).unwrap();
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(id, TaskState::Blocked, &mut rq).unwrap();

    let res = rq.enqueue(id, &tm);
    let rejected = res == Err(QueueError::TaskNotReady(id, TaskState::Blocked));
    let len_zero = rq.len() == 0;

    let passed = rejected && len_zero;
    if passed {
        info!("  [PASS] RQ-REG-2: Blocked task rejected by public enqueue API (TaskNotReady).");
    } else {
        error!("  [FAIL] RQ-REG-2: Blocked task was not rejected by public enqueue API!");
    }
    passed
}

/// Regression Test 3: A Terminated task cannot be inserted through the public API.
fn test_reg_terminated_task_cannot_be_enqueued() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("TermTask", None).unwrap();
    tm.transition_task_sync(id, TaskState::Terminated, &mut rq).unwrap();

    let res = rq.enqueue(id, &tm);
    let rejected = res == Err(QueueError::TaskNotReady(id, TaskState::Terminated));
    let len_zero = rq.len() == 0;

    let passed = rejected && len_zero;
    if passed {
        info!("  [PASS] RQ-REG-3: Terminated task rejected by public enqueue API (TaskNotReady).");
    } else {
        error!("  [FAIL] RQ-REG-3: Terminated task was not rejected by public enqueue API!");
    }
    passed
}

/// Regression Test 4: A nonexistent TaskId cannot be inserted.
fn test_reg_nonexistent_task_id_rejected() -> bool {
    let tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let ghost_id = TaskId(999999);

    let res = rq.enqueue(ghost_id, &tm);
    let rejected = res == Err(QueueError::TaskNotFound(ghost_id));
    let len_zero = rq.len() == 0;

    let passed = rejected && len_zero;
    if passed {
        info!("  [PASS] RQ-REG-4: Nonexistent TaskId rejected (TaskNotFound).");
    } else {
        error!("  [FAIL] RQ-REG-4: Nonexistent TaskId was not rejected!");
    }
    passed
}

/// Regression Test 5: A Ready task can be inserted.
fn test_reg_ready_task_can_be_enqueued() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("ReadyTask", None).unwrap();

    let res = rq.enqueue(id, &tm);
    let accepted = res.is_ok();
    let in_queue = rq.contains(id) && rq.len() == 1;

    let passed = accepted && in_queue;
    if passed {
        info!("  [PASS] RQ-REG-5: Ready task successfully validated and enqueued.");
    } else {
        error!("  [FAIL] RQ-REG-5: Ready task failed to enqueue!");
    }
    passed
}

/// Regression Test 6: Ready -> Running automatically removes the task.
fn test_reg_auto_sync_ready_to_running_removes() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("T-R2Run", None).unwrap();
    rq.enqueue(id, &tm).unwrap();
    assert!(rq.contains(id) && rq.len() == 1);

    // Transition to Running
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();
    let removed = !rq.contains(id) && rq.len() == 0;

    if removed {
        info!("  [PASS] RQ-REG-6: Ready -> Running automatically removed task from Ready Queue.");
    } else {
        error!("  [FAIL] RQ-REG-6: Task was not removed on Ready -> Running!");
    }
    removed
}

/// Regression Test 7: Task lifecycle (Ready -> Running -> Blocked -> Ready) with Ready Queue auto-sync.
fn test_reg_auto_sync_lifecycle_and_ready_blocked_rejected() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("T-Lifecycle", None).unwrap();

    // 1. A Ready task is inserted into the Ready Queue
    rq.enqueue(id, &tm).unwrap();
    let step1_in_queue = rq.contains(id) && rq.len() == 1;

    // 2. It transitions to Running (auto-sync removes it from Ready Queue)
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();
    let step2_removed_on_run = !rq.contains(id) && rq.len() == 0;

    // 3. It transitions from Running to Blocked
    tm.transition_task_sync(id, TaskState::Blocked, &mut rq).unwrap();
    // 4. It remains absent from the Ready Queue
    let step4_not_in_queue = !rq.contains(id) && rq.len() == 0;

    // 5. It transitions from Blocked to Ready
    tm.transition_task_sync(id, TaskState::Ready, &mut rq).unwrap();
    // 6. It is automatically appended to the queue tail
    let step6_appended_tail = rq.contains(id) && rq.len() == 1;

    // 7. Verify direct Ready -> Blocked transition is strictly rejected
    let mut t_direct = Task::new("DirectBlock", None);
    let direct_err = t_direct.transition_to(TaskState::Blocked);
    let step7_direct_rejected = direct_err == Err(StateTransitionError {
        from: TaskState::Ready,
        to: TaskState::Blocked,
    });

    let passed = step1_in_queue
        && step2_removed_on_run
        && step4_not_in_queue
        && step6_appended_tail
        && step7_direct_rejected;

    if passed {
        info!("  [PASS] RQ-REG-7: Lifecycle (Ready->Running->Blocked->Ready) auto-sync verified; Ready->Blocked rejected.");
    } else {
        error!("  [FAIL] RQ-REG-7: Lifecycle auto-sync or Ready->Blocked rejection failed!");
    }
    passed
}

/// Regression Test 8: Ready -> Terminated automatically removes the task.
fn test_reg_auto_sync_ready_to_terminated_removes() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();
    let id = tm.create_task("T-R2Term", None).unwrap();
    rq.enqueue(id, &tm).unwrap();
    assert!(rq.contains(id) && rq.len() == 1);

    // Transition to Terminated
    tm.transition_task_sync(id, TaskState::Terminated, &mut rq).unwrap();
    let removed = !rq.contains(id) && rq.len() == 0;

    if removed {
        info!("  [PASS] RQ-REG-8: Ready -> Terminated automatically removed task from Ready Queue.");
    } else {
        error!("  [FAIL] RQ-REG-8: Task was not removed on Ready -> Terminated!");
    }
    removed
}

/// Regression Test 9: Blocked -> Ready automatically inserts the task at the queue tail.
fn test_reg_auto_sync_blocked_to_ready_appends_tail() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("T1-Head", None).unwrap();
    rq.enqueue(id1, &tm).unwrap();

    let id2 = tm.create_task("T2-Blocked", None).unwrap();
    tm.transition_task_sync(id2, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(id2, TaskState::Blocked, &mut rq).unwrap();

    // Transition id2 from Blocked to Ready
    tm.transition_task_sync(id2, TaskState::Ready, &mut rq).unwrap();

    let len_2 = rq.len() == 2;
    let d1 = rq.dequeue();
    let d2 = rq.dequeue();
    let fifo_tail = d1 == Some(id1) && d2 == Some(id2);

    let passed = len_2 && fifo_tail;
    if passed {
        info!("  [PASS] RQ-REG-9: Blocked -> Ready automatically enqueued task at queue tail.");
    } else {
        error!("  [FAIL] RQ-REG-9: Blocked -> Ready did not append to queue tail!");
    }
    passed
}

/// Regression Test 10: Running -> Ready automatically inserts the task at the queue tail.
fn test_reg_auto_sync_running_to_ready_appends_tail() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("T1-Head", None).unwrap();
    rq.enqueue(id1, &tm).unwrap();

    let id2 = tm.create_task("T2-Running", None).unwrap();
    tm.transition_task_sync(id2, TaskState::Running, &mut rq).unwrap();

    // Preempt id2 from Running to Ready
    tm.transition_task_sync(id2, TaskState::Ready, &mut rq).unwrap();

    let len_2 = rq.len() == 2;
    let d1 = rq.dequeue();
    let d2 = rq.dequeue();
    let fifo_tail = d1 == Some(id1) && d2 == Some(id2);

    let passed = len_2 && fifo_tail;
    if passed {
        info!("  [PASS] RQ-REG-10: Running -> Ready (preemption) automatically enqueued task at queue tail.");
    } else {
        error!("  [FAIL] RQ-REG-10: Running -> Ready did not append to queue tail!");
    }
    passed
}

/// Regression Test 11: Repeated Ready transitions do not create duplicates.
fn test_reg_auto_sync_repeated_ready_no_duplicate() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id = tm.create_task("T-Repeat", None).unwrap();
    rq.enqueue(id, &tm).unwrap();
    assert_eq!(rq.len(), 1);

    // Repeated Ready -> Ready transition
    tm.transition_task_sync(id, TaskState::Ready, &mut rq).unwrap();

    let len_one = rq.len() == 1;
    let d1 = rq.dequeue();
    let d2 = rq.dequeue();
    let no_dup = d1 == Some(id) && d2 == None;

    let passed = len_one && no_dup;
    if passed {
        info!("  [PASS] RQ-REG-11: Repeated Ready transitions do not create duplicate queue entries.");
    } else {
        error!("  [FAIL] RQ-REG-11: Duplicate entry created on repeated Ready transition!");
    }
    passed
}

/// Regression Test 12: A failed transition leaves both task state and queue unchanged.
fn test_reg_auto_sync_failed_transition_preserves_state_and_queue() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id = tm.create_task("T-FailCheck", None).unwrap();
    rq.enqueue(id, &tm).unwrap();

    // Transition to Running, then Terminated
    tm.transition_task_sync(id, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(id, TaskState::Terminated, &mut rq).unwrap();

    assert_eq!(rq.len(), 0);
    assert_eq!(tm.get_task(id).unwrap().state, TaskState::Terminated);

    // Attempt illegal transition Terminated -> Running
    let fail_res = tm.transition_task_sync(id, TaskState::Running, &mut rq);
    let is_err = fail_res.is_err();

    // Verify task state is STILL Terminated
    let state_preserved = tm.get_task(id).unwrap().state == TaskState::Terminated;

    // Verify queue is STILL empty
    let queue_untouched = rq.len() == 0 && !rq.contains(id);

    let passed = is_err && state_preserved && queue_untouched;
    if passed {
        info!("  [PASS] RQ-REG-12: Failed transition leaves both task state and Ready Queue unchanged.");
    } else {
        error!("  [FAIL] RQ-REG-12: Failed transition corrupted state or queue!");
    }
    passed
}

/// Regression Test 13: Queue-full transition failure safely preserves task state and queue contents.
fn test_reg_auto_sync_queue_full_transition_fails_safely() -> bool {
    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    // 1. Fill the Ready Queue to capacity (32 tasks) using raw enqueue
    let mut expected_ids = [TaskId(0); QUEUE_CAPACITY];
    for i in 0..QUEUE_CAPACITY {
        let fake_id = TaskId((1000 + i) as u64);
        rq.enqueue_raw(fake_id).unwrap();
        expected_ids[i] = fake_id;
    }
    let is_full = rq.is_full() && rq.len() == QUEUE_CAPACITY;

    // 2. Create a Blocked task in TaskManager (Ready -> Running -> Blocked)
    let blocked_id = tm.create_task("BlockedWorker", None).unwrap();
    tm.transition_task_sync(blocked_id, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(blocked_id, TaskState::Blocked, &mut rq).unwrap();
    assert_eq!(tm.get_task(blocked_id).unwrap().state, TaskState::Blocked);

    // 3. Attempt Blocked -> Ready while ReadyQueue is full
    let trans_res = tm.transition_task_sync(blocked_id, TaskState::Ready, &mut rq);

    // 4. Verify the operation fails safely with QueueFull error
    let failed_safely = trans_res == Err(TaskManagerError::QueueError(QueueError::QueueFull));

    // 5. Verify the task remains in Blocked state
    let task_still_blocked = tm.get_task(blocked_id).unwrap().state == TaskState::Blocked;

    // 6. Verify the queue still contains exactly the original 32 entries in their original order
    let len_still_32 = rq.len() == QUEUE_CAPACITY;
    let mut order_preserved = true;
    for i in 0..QUEUE_CAPACITY {
        if rq.dequeue() != Some(expected_ids[i]) {
            order_preserved = false;
            break;
        }
    }
    let queue_clean_after = rq.is_empty();

    let passed = is_full
        && failed_safely
        && task_still_blocked
        && len_still_32
        && order_preserved
        && queue_clean_after;

    if passed {
        info!("  [PASS] RQ-REG-13: Queue-full transition failure safely preserves task state and queue contents.");
    } else {
        error!("  [FAIL] RQ-REG-13: Queue-full transition did not fail safely or corrupted state/queue!");
    }
    passed
}
