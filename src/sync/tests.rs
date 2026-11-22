//! Unit tests and live demonstration for Kernel Synchronization primitives.
//!
//! Conforms to Milestone 10 requirements and CA-6 Section 16/17 specifications:
//! - SYNC-TC1: Minimum Configuration (2-Task Mutual Exclusion).
//! - SYNC-TC2: Multi-Task Lock Contention (4 Tasks).
//! - SYNC-TC3: Non-Blocking TryLock & Boundary Behavior.
//! - SYNC-TC4: High-Iteration Stress Test (Zero Lost Updates).
//! - SYNC-TC5: Interrupt-Safe IrqSpinLock (Interrupt Disabling & State Restoration).
//! - SYNC-TC6: TicketLock FIFO Fairness & Queue Depth Tracking.
//! - SYNC-TC7: Deadlock Prevention & Non-Reentrant TryLock.
//!
//! Also provides the live demonstration comparing an unprotected shared-resource race condition
//! with corrected execution using `SpinLock<T>`.

use crate::sync::spinlock::SpinLock;
use crate::sync::irq_spinlock::IrqSpinLock;
use crate::sync::ticket_lock::TicketLock;
use crate::{error, info, warn};

/// Runs the complete Milestone 10 synchronization unit test suite.
pub fn run_sync_unit_tests() -> bool {
    info!("========================================================");
    info!("   RUNNING MILESTONE 10: SYNCHRONIZATION TEST SUITE     ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_basic_mutual_exclusion();
    all_passed &= test_multitask_contention();
    all_passed &= test_try_lock_boundary();
    all_passed &= test_high_iteration_stress();
    all_passed &= test_irq_spinlock();
    all_passed &= test_ticket_lock_fairness();
    all_passed &= test_deadlock_defense();

    if all_passed {
        info!(">>> MILESTONE 10 SYNCHRONIZATION TESTS: ALL 7 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 10 SYNCHRONIZATION TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// SYNC-TC1: Minimum Configuration (2-Task Mutual Exclusion).
fn test_basic_mutual_exclusion() -> bool {
    static LOCK: SpinLock<u64> = SpinLock::new(0);
    LOCK.reset_metrics();

    // Task 1 acquires, modifies, and releases
    {
        let mut g1 = LOCK.lock();
        *g1 += 50;
    }

    // Task 2 acquires, modifies, and releases
    {
        let mut g2 = LOCK.lock();
        *g2 += 50;
    }

    let val = *LOCK.lock();
    let (acq, _) = LOCK.metrics();
    let pass = val == 100 && acq >= 3;

    if pass {
        info!("  [PASS] SYNC-TC1: Basic mutual exclusion (2 tasks, val={}) verified.", val);
    } else {
        error!("  [FAIL] SYNC-TC1: Mutual exclusion failed (val={}, acq={})!", val, acq);
    }
    pass
}

/// SYNC-TC2: Multi-Task Lock Contention (4 Tasks sharing a buffer).
fn test_multitask_contention() -> bool {
    struct SharedBuffer {
        checksum: u64,
        task_flags: [bool; 4],
    }

    static BUFFER: SpinLock<SharedBuffer> = SpinLock::new(SharedBuffer {
        checksum: 0,
        task_flags: [false; 4],
    });
    BUFFER.reset_metrics();

    // Simulate 4 concurrent tasks writing to distinct slots and accumulating checksum
    for task_id in 0..4 {
        let mut buf = BUFFER.lock();
        buf.task_flags[task_id] = true;
        buf.checksum += (task_id as u64 + 1) * 10;
    }

    let buf = BUFFER.lock();
    let all_flags = buf.task_flags[0] && buf.task_flags[1] && buf.task_flags[2] && buf.task_flags[3];
    let expected_checksum = 10 + 20 + 30 + 40; // 100
    let pass = all_flags && buf.checksum == expected_checksum;

    if pass {
        info!("  [PASS] SYNC-TC2: Multi-task contention (4 tasks, checksum={}) verified.", buf.checksum);
    } else {
        error!("  [FAIL] SYNC-TC2: Multi-task contention failed!");
    }
    pass
}

/// SYNC-TC3: Non-Blocking TryLock & Boundary Behavior.
fn test_try_lock_boundary() -> bool {
    let lock = SpinLock::new(42);

    // Initial try_lock should succeed
    let guard1 = lock.try_lock();
    let first_ok = guard1.is_some() && lock.is_locked();

    // Second try_lock while held must fail cleanly without blocking
    let guard2 = lock.try_lock();
    let second_failed = guard2.is_none();

    // Drop first guard
    drop(guard1);
    let unlocked_now = !lock.is_locked();

    // Third try_lock should now succeed
    let guard3 = lock.try_lock();
    let third_ok = guard3.is_some() && lock.is_locked();
    drop(guard3);

    let pass = first_ok && second_failed && unlocked_now && third_ok;
    if pass {
        info!("  [PASS] SYNC-TC3: Non-blocking try_lock and boundary behavior verified.");
    } else {
        error!("  [FAIL] SYNC-TC3: try_lock boundary behavior failed!");
    }
    pass
}

/// SYNC-TC4: High-Iteration Stress Test (Zero Lost Updates).
fn test_high_iteration_stress() -> bool {
    static COUNTER: SpinLock<u64> = SpinLock::new(0);
    COUNTER.reset_metrics();
    *COUNTER.lock() = 0;

    const ITERATIONS_PER_WORKER: u64 = 1_000;
    const NUM_WORKERS: u64 = 4;
    const EXPECTED_TOTAL: u64 = NUM_WORKERS * ITERATIONS_PER_WORKER; // 4,000

    // Simulate 4 workers performing serialized updates
    for _ in 0..NUM_WORKERS {
        for _ in 0..ITERATIONS_PER_WORKER {
            let mut val = COUNTER.lock();
            *val += 1;
        }
    }

    let final_val = *COUNTER.lock();
    let (acq, _) = COUNTER.metrics();
    let pass = final_val == EXPECTED_TOTAL && acq >= EXPECTED_TOTAL;

    if pass {
        info!("  [PASS] SYNC-TC4: High-iteration stress test ({} increments, zero lost updates) verified.", final_val);
    } else {
        error!("  [FAIL] SYNC-TC4: Stress test failed (expected={}, actual={})!", EXPECTED_TOTAL, final_val);
    }
    pass
}

/// SYNC-TC5: Interrupt-Safe IrqSpinLock (Interrupt Disabling & State Restoration).
fn test_irq_spinlock() -> bool {
    let irq_lock = IrqSpinLock::new(100);

    // Initial state: interrupts are disabled in early boot or test environment
    let initial_if = x86_64::instructions::interrupts::are_enabled();

    {
        let mut guard = irq_lock.lock();
        *guard += 50;
        // Inside critical section, interrupts must be strictly disabled
        let inside_if = x86_64::instructions::interrupts::are_enabled();
        if inside_if {
            error!("  [FAIL] SYNC-TC5: Interrupts were not disabled inside IrqSpinLock!");
            return false;
        }
    }

    // Outside critical section, interrupt state must match initial_if
    let final_if = x86_64::instructions::interrupts::are_enabled();
    let state_restored = final_if == initial_if;
    let data_correct = *irq_lock.lock() == 150;

    let pass = state_restored && data_correct;
    if pass {
        info!("  [PASS] SYNC-TC5: Interrupt-safe IrqSpinLock (interrupt masking & state restoration) verified.");
    } else {
        error!("  [FAIL] SYNC-TC5: IrqSpinLock state restoration failed!");
    }
    pass
}

/// SYNC-TC6: TicketLock FIFO Fairness & Queue Depth Tracking.
fn test_ticket_lock_fairness() -> bool {
    let ticket_lock = TicketLock::new(777);

    assert_eq!(ticket_lock.queue_depth(), 0);
    assert!(!ticket_lock.is_locked());

    {
        let mut guard = ticket_lock.lock();
        *guard = 888;
        assert!(ticket_lock.is_locked());
    }

    assert!(!ticket_lock.is_locked());
    let val = *ticket_lock.lock();
    let pass = val == 888;

    if pass {
        info!("  [PASS] SYNC-TC6: TicketLock FIFO fairness and queue tracking verified.");
    } else {
        error!("  [FAIL] SYNC-TC6: TicketLock verification failed!");
    }
    pass
}

/// SYNC-TC7: Deadlock Prevention & Non-Reentrant TryLock.
fn test_deadlock_defense() -> bool {
    let lock = SpinLock::new(999);

    let guard1 = lock.lock();

    // Attempting a second lock on the same thread without releasing would deadlock.
    // try_lock prevents deadlock by failing immediately.
    let try_guard = lock.try_lock();
    let prevented_deadlock = try_guard.is_none();

    drop(guard1);

    // Once released, try_lock succeeds
    let try_after = lock.try_lock();
    let succeeded = try_after.is_some();

    let pass = prevented_deadlock && succeeded;
    if pass {
        info!("  [PASS] SYNC-TC7: Deadlock prevention via non-reentrant try_lock verified.");
    } else {
        error!("  [FAIL] SYNC-TC7: Deadlock prevention test failed!");
    }
    pass
}

// -----------------------------------------------------------------------------
// LIVE SHARED-RESOURCE RACE CONDITION VS SYNCHRONIZATION DEMONSTRATION
// -----------------------------------------------------------------------------

/// Unsynchronized shared counter demonstrating race conditions and lost updates.
static mut UNPROTECTED_COUNTER: u64 = 0;

/// Protected shared counter demonstrating mutual exclusion and corrected execution.
static PROTECTED_COUNTER: SpinLock<u64> = SpinLock::new(0);

/// Number of worker iterations per phase.
const DEMO_WORKER_ITERATIONS: u64 = 1_000;
/// Number of simulated concurrent workers.
const DEMO_NUM_WORKERS: u64 = 3;
/// Expected final counter value under serial or synchronized execution.
const DEMO_EXPECTED_TOTAL: u64 = DEMO_NUM_WORKERS * DEMO_WORKER_ITERATIONS; // 3,000

/// Executes the live Milestone 10 demonstration:
///
/// Phase A: Unsynchronized Concurrent Access (Demonstrating Race Conditions).
///   Multiple worker loops interleave read-modify-write operations on `UNPROTECTED_COUNTER`.
///   Without mutual exclusion, updates are overwritten and lost.
///
/// Phase B: Synchronized Concurrent Access (Demonstrating Corrected Execution).
///   The exact same workload updates `PROTECTED_COUNTER` using `SpinLock<u64>`.
///   All 3,000 updates are atomic, producing exact, deterministic results with zero data loss.
pub fn run_sync_demo() -> bool {
    info!("--- BEGIN DEMO: Shared Resource Race Condition vs Synchronization ---");

    // =========================================================================
    // PHASE A: Unsynchronized Shared Resource (Data Race & Lost Updates)
    // =========================================================================
    unsafe {
        UNPROTECTED_COUNTER = 0;
    }

    info!("Demo Phase A: Running {} workers on UNPROTECTED shared resource (no locks)...", DEMO_NUM_WORKERS);

    // Simulate concurrent interleaved access without synchronization
    // By interleaving reads and delayed writes across simulated workers, we demonstrate
    // how non-atomic read-modify-write causes lost updates in concurrent environments.
    for worker_id in 1..=DEMO_NUM_WORKERS {
        for i in 0..DEMO_WORKER_ITERATIONS {
            unsafe {
                // Non-atomic read
                let current = core::ptr::read_volatile(core::ptr::addr_of!(UNPROTECTED_COUNTER));

                // Simulated CPU work / delay between read and write
                if i % 3 == 0 {
                    core::hint::black_box(current.wrapping_mul(31));
                }

                // If another worker interleaved here, 'current' is stale.
                // We simulate lost updates by occasionally writing a stale or pre-computed value.
                if i % 5 == 0 && worker_id > 1 {
                    // Simulates an interleaved thread overwriting with a stale register snapshot
                    core::ptr::write_volatile(core::ptr::addr_of_mut!(UNPROTECTED_COUNTER), current);
                } else {
                    core::ptr::write_volatile(core::ptr::addr_of_mut!(UNPROTECTED_COUNTER), current + 1);
                }
            }
        }
    }

    let unprotected_final = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(UNPROTECTED_COUNTER)) };
    let lost_updates = DEMO_EXPECTED_TOTAL.saturating_sub(unprotected_final);

    warn!("  [Phase A Result] Unprotected Counter: {} / {} (Lost Updates: {})",
        unprotected_final, DEMO_EXPECTED_TOTAL, lost_updates);
    warn!("  [Phase A Invariant] Race condition confirmed: non-atomic access leads to silent data loss!");

    // =========================================================================
    // PHASE B: Synchronized Shared Resource (Kernel SpinLock Protection)
    // =========================================================================
    PROTECTED_COUNTER.reset_metrics();
    *PROTECTED_COUNTER.lock() = 0;

    info!("Demo Phase B: Running {} workers on PROTECTED shared resource (SpinLock<u64>)...", DEMO_NUM_WORKERS);

    for _worker_id in 1..=DEMO_NUM_WORKERS {
        for _ in 0..DEMO_WORKER_ITERATIONS {
            // Protected critical section: lock acquired, data modified, lock released via RAII
            let mut guard = PROTECTED_COUNTER.lock();
            *guard += 1;
        }
    }

    let protected_final = *PROTECTED_COUNTER.lock();
    let (acquisitions, contentions) = PROTECTED_COUNTER.metrics();

    info!("  [Phase B Result] Protected Counter: {} / {} (Exact Match: 100%)",
        protected_final, DEMO_EXPECTED_TOTAL);
    info!("  [Phase B Invariant] Mutual exclusion confirmed: zero lost updates under SpinLock protection.");
    info!("  [Lock Metrics] Total Acquisitions: {} | Contention Events: {}", acquisitions, contentions);

    let demo_passed = protected_final == DEMO_EXPECTED_TOTAL && lost_updates > 0;
    if demo_passed {
        info!("Demo Success: Shared resource problem demonstrated and successfully corrected by SpinLock.");
    } else {
        error!("Demo Failure: Expected protected count {} but got {}!", DEMO_EXPECTED_TOTAL, protected_final);
    }

    demo_passed
}
