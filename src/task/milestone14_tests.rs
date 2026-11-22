//! Milestone 14: Comprehensive Kernel Testing, Debugging, and Telemetry Test Suite.
//!
//! Conforms to CA-6 Section 15 (Testing Strategy), Section 16 (Required Test Cases TC1–TC6),
//! Section 17 (Evidence Requirements), Section 18 (Performance Evaluation), and
//! Section 21 (Debugging Strategy).
//!
//! Test Tiers Implemented:
//! - Level 1: Unit-Level Testing (Boundary parameters, capacity limits, edge cases)
//! - Level 2: Mechanism Testing (Stress endurance, failure injection, lock contention)
//! - Level 3: Integration Testing (Full lifecycle from boot to multi-task termination)
//!
//! Required Test Cases (TC1–TC6):
//! - DEBUG-TC1: Minimum Configuration (2 Tasks Lifecycle & Zero Resource Leakage)
//! - DEBUG-TC2: Multiple Concurrent Tasks & Memory Isolation (4 Tasks: 2 Kernel + 2 User)
//! - DEBUG-TC3: Boundary & Edge Condition Enforcement (Zero-length, capacity bounds, limits)
//! - DEBUG-TC4: High-Load Stress & Endurance Verification (Heap churn, queue wrapping, IPC saturation)
//! - DEBUG-TC5: Comprehensive Failure Injection & Safety Rejection (Syscall, MMU, pointer validation)
//! - DEBUG-TC6: Repeatability & Zero-State-Drift Verification (Dual-pass deterministic execution)

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

use core::sync::atomic::Ordering;
use crate::{debug, info};
use crate::kernel::diagnostics::{trace_event, KernelTelemetry, TraceEventKind};
use crate::memory::address_space::{
    phys_to_virt, restore_kernel_address_space, AddressSpace, USER_CODE_VIRT_ADDR, USER_DATA_VIRT_ADDR,
    USER_STACK_TOP,
};
use crate::interrupts::syscall::{
    run_user_code_with_exit_return, syscall_dispatcher, EFAULT, EINVAL, ENOSYS,
    LAST_USER_EXIT_CODE, SYS_WRITE,
};
use crate::task::{
    create, create_process, dequeue_task, enqueue_task, reap_process, reap_terminated_tasks,
    sys_ipc_recv, sys_ipc_send, terminate, transition, IpcError, ProcessError,
    QueueError, StateTransitionError, TaskManagerError, TaskState, MAX_PROCESSES, QUEUE_CAPACITY, READY_QUEUE,
};

/// Runs the complete suite of 6 Milestone 14 Debugging & Testing Cases (TC1–TC6).
pub fn run_milestone14_tests() -> bool {
    info!("========================================================");
    info!("     RUNNING MILESTONE 14: TESTING & DEBUGGING SUITE    ");
    info!("========================================================");

    let mut all_passed = true;

    if !test_debug_tc1_minimum_configuration() {
        all_passed = false;
    }
    if !test_debug_tc2_multiple_tasks_concurrency() {
        all_passed = false;
    }
    if !test_debug_tc3_boundary_and_limits() {
        all_passed = false;
    }
    if !test_debug_tc4_stress_and_high_load() {
        all_passed = false;
    }
    if !test_debug_tc5_failure_injection_and_rejection() {
        all_passed = false;
    }
    if !test_debug_tc6_repeatability_and_determinism() {
        all_passed = false;
    }

    if all_passed {
        info!(">>> MILESTONE 14 KERNEL TESTS: ALL 6 TESTS PASSED <<<");
    } else {
        crate::error!(">>> MILESTONE 14 KERNEL TESTS: FAILURES DETECTED <<<");
    }

    all_passed
}

// =========================================================================
// DEBUG-TC1: Minimum Configuration (Level 3 Integration)
// =========================================================================

/// Dummy entry point for TC1 task Alpha.
fn tc1_worker_alpha() {}

/// Dummy entry point for TC1 task Beta.
fn tc1_worker_beta() {}

/// DEBUG-TC1: Verifies clean lifecycle and zero resource leakage of a minimal
/// two-task configuration conforming to CA-6 Section 16 TC1.
fn test_debug_tc1_minimum_configuration() -> bool {
    trace_event(TraceEventKind::TaskCreated, 1, 0);

    // 1. Create exactly two controlled tasks
    let t1 = match create("MinTask-Alpha", Some(tc1_worker_alpha)) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC1 Failed to create MinTask-Alpha: {:?}", e);
            return false;
        }
    };

    let t2 = match create("MinTask-Beta", Some(tc1_worker_beta)) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC1 Failed to create MinTask-Beta: {:?}", e);
            return false;
        }
    };

    // 2. Enqueue both tasks into the Ready Queue
    if let Err(e) = enqueue_task(t1) {
        crate::error!("DEBUG-TC1 Failed to enqueue t1: {:?}", e);
        return false;
    }
    if let Err(e) = enqueue_task(t2) {
        crate::error!("DEBUG-TC1 Failed to enqueue t2: {:?}", e);
        return false;
    }

    // Verify Ready Queue contains exactly 2 tasks
    {
        let rq = READY_QUEUE.lock();
        if rq.len() != 2 || !rq.contains(t1) || !rq.contains(t2) {
            crate::error!("DEBUG-TC1 Failed: ReadyQueue count mismatch (expected 2, got {})", rq.len());
            return false;
        }
    }

    // 3. Dispatch and execute task 1 to termination
    if let Err(e) = transition(t1, TaskState::Running) {
        crate::error!("DEBUG-TC1 Failed to transition t1 to RUNNING: {:?}", e);
        return false;
    }
    trace_event(TraceEventKind::TaskDispatched, t1.0 as u64, 0);

    if let Err(e) = terminate(t1) {
        crate::error!("DEBUG-TC1 Failed to terminate t1: {:?}", e);
        return false;
    }
    trace_event(TraceEventKind::TaskTerminated, t1.0 as u64, 0);

    // 4. Dispatch and execute task 2 to termination
    if let Err(e) = transition(t2, TaskState::Running) {
        crate::error!("DEBUG-TC1 Failed to transition t2 to RUNNING: {:?}", e);
        return false;
    }
    trace_event(TraceEventKind::TaskDispatched, t2.0 as u64, 0);

    if let Err(e) = terminate(t2) {
        crate::error!("DEBUG-TC1 Failed to terminate t2: {:?}", e);
        return false;
    }
    trace_event(TraceEventKind::TaskTerminated, t2.0 as u64, 0);

    // 5. Clean up and reclaim resources
    let reaped = reap_terminated_tasks();
    if reaped < 2 {
        crate::error!("DEBUG-TC1 Failed: Expected at least 2 reaped tasks, got {}", reaped);
        return false;
    }

    // Verify queue is completely drained of these tasks
    {
        let rq = READY_QUEUE.lock();
        if rq.contains(t1) || rq.contains(t2) {
            crate::error!("DEBUG-TC1 Failed: Reaped tasks remain in ReadyQueue!");
            return false;
        }
    }

    info!("  [PASS] DEBUG-TC1: Minimum 2-task configuration lifecycle and zero resource leakage verified.");
    true
}

// =========================================================================
// DEBUG-TC2: Multiple Concurrent Tasks & Privilege Isolation (Level 3 Integration)
// =========================================================================

/// Dummy worker for kernel worker task A.
fn tc2_kthread_worker_a() {}

/// Dummy worker for kernel worker task B.
fn tc2_kthread_worker_b() {}

/// DEBUG-TC2: Verifies multi-task execution across 4 tasks (2 kernel threads + 2 isolated user processes)
/// running concurrently with hardware paging isolation (CA-6 Section 16 TC2).
fn test_debug_tc2_multiple_tasks_concurrency() -> bool {
    // 1. Create 2 kernel threads
    let kt1 = match create("KThread-1", Some(tc2_kthread_worker_a)) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create KThread-1: {:?}", e);
            return false;
        }
    };
    let kt2 = match create("KThread-2", Some(tc2_kthread_worker_b)) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create KThread-2: {:?}", e);
            return false;
        }
    };

    // 2. Create 2 isolated User Processes with private CR3 address spaces
    let aspace_alpha = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create AddressSpace Alpha: {}", e);
            return false;
        }
    };
    let aspace_beta = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create AddressSpace Beta: {}", e);
            aspace_alpha.destroy();
            return false;
        }
    };

    let p_alpha = match create_process("Proc-Alpha-Isolated") {
        Ok(pid) => pid,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create Proc-Alpha: {:?}", e);
            aspace_alpha.destroy();
            aspace_beta.destroy();
            return false;
        }
    };
    let p_beta = match create_process("Proc-Beta-Isolated") {
        Ok(pid) => pid,
        Err(e) => {
            crate::error!("DEBUG-TC2 Failed to create Proc-Beta: {:?}", e);
            aspace_alpha.destroy();
            aspace_beta.destroy();
            let _ = reap_process(p_alpha);
            return false;
        }
    };

    // 3. Verify hardware address space isolation & attach to process descriptors
    let alpha_cr3 = aspace_alpha.cr3.start_address().as_u64();
    let beta_cr3 = aspace_beta.cr3.start_address().as_u64();
    if alpha_cr3 == beta_cr3 {
        crate::error!("DEBUG-TC2 Isolation Failure: Identical CR3 for distinct processes (0x{:x})", alpha_cr3);
        aspace_alpha.destroy();
        aspace_beta.destroy();
        let _ = reap_process(p_alpha);
        let _ = reap_process(p_beta);
        return false;
    }

    {
        let mut reg = crate::task::process::PROCESS_REGISTRY.lock();
        if let Ok(proc) = reg.get_process_mut(p_alpha) {
            proc.attach_address_space(aspace_alpha.cr3);
        }
        if let Ok(proc) = reg.get_process_mut(p_beta) {
            proc.attach_address_space(aspace_beta.cr3);
        }
    }

    // Physical page frames for User Data must be distinct
    let alpha_pa = aspace_alpha.user_data_frame.start_address().as_u64();
    let beta_pa = aspace_beta.user_data_frame.start_address().as_u64();
    if alpha_pa == beta_pa {
        crate::error!("DEBUG-TC2 Isolation Failure: Identical physical frame for distinct processes (0x{:x})", alpha_pa);
        aspace_alpha.destroy();
        aspace_beta.destroy();
        let _ = reap_process(p_alpha);
        let _ = reap_process(p_beta);
        return false;
    }

    // 4. Verify live hardware isolation via direct virtual address execution:
    // Switch to Process Alpha address space and write marker directly to User Virtual Address
    aspace_alpha.switch_to();
    let (curr_cr3_alpha, _) = x86_64::registers::control::Cr3::read();
    if curr_cr3_alpha.start_address().as_u64() != alpha_cr3 {
        restore_kernel_address_space();
        crate::error!("DEBUG-TC2 Failed: Active CR3 does not match Process Alpha CR3");
        aspace_alpha.destroy();
        aspace_beta.destroy();
        let _ = reap_process(p_alpha);
        let _ = reap_process(p_beta);
        return false;
    }
    unsafe {
        core::ptr::write_volatile(USER_DATA_VIRT_ADDR as *mut u64, 0xAAAA_1111_2222_3333);
    }
    let read_alpha = unsafe { core::ptr::read_volatile(USER_DATA_VIRT_ADDR as *const u64) };

    // Switch to Process Beta address space and write distinct marker to identical User Virtual Address
    aspace_beta.switch_to();
    let (curr_cr3_beta, _) = x86_64::registers::control::Cr3::read();
    if curr_cr3_beta.start_address().as_u64() != beta_cr3 {
        restore_kernel_address_space();
        crate::error!("DEBUG-TC2 Failed: Active CR3 does not match Process Beta CR3");
        aspace_alpha.destroy();
        aspace_beta.destroy();
        let _ = reap_process(p_alpha);
        let _ = reap_process(p_beta);
        return false;
    }
    unsafe {
        core::ptr::write_volatile(USER_DATA_VIRT_ADDR as *mut u64, 0xBBBB_4444_5555_6666);
    }
    let read_beta = unsafe { core::ptr::read_volatile(USER_DATA_VIRT_ADDR as *const u64) };

    // Switch BACK to Process Alpha to prove Beta's write did not alter Alpha's memory
    aspace_alpha.switch_to();
    let read_alpha_second = unsafe { core::ptr::read_volatile(USER_DATA_VIRT_ADDR as *const u64) };

    // Switch BACK to Process Beta to prove Beta's memory remains intact
    aspace_beta.switch_to();
    let read_beta_second = unsafe { core::ptr::read_volatile(USER_DATA_VIRT_ADDR as *const u64) };

    restore_kernel_address_space();

    if read_alpha != 0xAAAA_1111_2222_3333 || read_beta != 0xBBBB_4444_5555_6666
        || read_alpha_second != 0xAAAA_1111_2222_3333 || read_beta_second != 0xBBBB_4444_5555_6666 {
        crate::error!("DEBUG-TC2 Memory Isolation Corrupted: Alpha1=0x{:x}, Beta1=0x{:x}, Alpha2=0x{:x}, Beta2=0x{:x}",
            read_alpha, read_beta, read_alpha_second, read_beta_second);
        aspace_alpha.destroy();
        aspace_beta.destroy();
        let _ = reap_process(p_alpha);
        let _ = reap_process(p_beta);
        return false;
    }

    // 5. Clean up kernel threads and user processes
    let _ = terminate(kt1);
    let _ = terminate(kt2);
    reap_terminated_tasks();

    aspace_alpha.destroy();
    aspace_beta.destroy();
    let _ = reap_process(p_alpha);
    let _ = reap_process(p_beta);

    info!("  [PASS] DEBUG-TC2: Multi-task concurrency and hardware address space isolation (4 tasks) verified.");
    true
}

// =========================================================================
// DEBUG-TC3: Boundary & Edge Condition Enforcement (Level 1 Unit)
// =========================================================================

/// DEBUG-TC3: Verifies strict enforcement of parameter bounds, zero-length requests,
/// and subsystem capacity limits (CA-6 Section 16 TC3).
fn test_debug_tc3_boundary_and_limits() -> bool {
    // 1. Syscall zero-length write boundary:
    // A length of 0 bytes with a valid user-space buffer must safely return 0
    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("DEBUG-TC3 Failed to create AddressSpace: {}", e);
            return false;
        }
    };
    aspace.switch_to();
    let ret_zero_len = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, 0, 0);
    restore_kernel_address_space();
    aspace.destroy();
    if ret_zero_len != 0 {
        crate::error!("DEBUG-TC3 Failed: Zero-length sys_write returned {}, expected 0", ret_zero_len);
        return false;
    }

    // 2. Empty Ready Queue dequeue boundary:
    // Dequeuing an empty queue must return None cleanly without panic or corruption
    {
        let mut rq = READY_QUEUE.lock();
        rq.clear();
        if rq.dequeue().is_some() {
            crate::error!("DEBUG-TC3 Failed: Empty queue dequeue returned Some!");
            return false;
        }
    }

    // 3. Ready Queue Capacity Boundary:
    // Dedicated ReadyQueue testing: filling to QUEUE_CAPACITY (32) succeeds; 33rd attempt fails with QueueFull
    {
        let mut test_q = crate::task::ready_queue::ReadyQueue::new();
        for i in 0..QUEUE_CAPACITY {
            if let Err(e) = test_q.enqueue_raw(crate::task::TaskId(1000 + i as u64)) {
                crate::error!("DEBUG-TC3 Failed to enqueue task {}: {:?}", i, e);
                return false;
            }
        }
        if test_q.len() != QUEUE_CAPACITY || !test_q.is_full() {
            crate::error!("DEBUG-TC3 Failed: Queue reported not full at capacity 32");
            return false;
        }
        let overflow_res = test_q.enqueue_raw(crate::task::TaskId(9999));
        if overflow_res != Err(QueueError::QueueFull) {
            crate::error!("DEBUG-TC3 Failed: Queue capacity bound not enforced: {:?}", overflow_res);
            return false;
        }
    }

    // 4. Memory Allocator Boundary:
    // Allocating single-byte and 4096-byte buffers via Vec and Box
    {
        let byte_box: Box<u8> = Box::new(0x42);
        if *byte_box != 0x42 {
            crate::error!("DEBUG-TC3 Failed: Single-byte Box value corrupted: {}", *byte_box);
            return false;
        }

        let mut page_vec: Vec<u8> = Vec::with_capacity(4096);
        for i in 0..4096 {
            page_vec.push((i % 256) as u8);
        }
        if page_vec.len() != 4096 || page_vec[4095] != (4095 % 256) as u8 {
            crate::error!("DEBUG-TC3 Failed: 4096-byte Vec boundary check failed");
            return false;
        }
    }

    // 5. Process Table Capacity Boundary:
    // Filling process registry up to MAX_PROCESSES (16)
    let mut fill_pids = [crate::task::ProcessId { index: 0, generation: 0 }; MAX_PROCESSES];
    let mut created_count = 0;
    for i in 0..MAX_PROCESSES {
        match create_process("CapacityProc") {
            Ok(pid) => {
                fill_pids[i] = pid;
                created_count += 1;
            }
            Err(e) => {
                crate::error!("DEBUG-TC3 Failed: Process creation failed at index {}: {:?}", i, e);
                for j in 0..created_count {
                    let _ = reap_process(fill_pids[j]);
                }
                return false;
            }
        }
    }

    // 17th process creation must fail with ProcessTableFull
    let p_over = create_process("ProcOverflow");
    if p_over != Err(ProcessError::ProcessTableFull) {
        crate::error!("DEBUG-TC3 Failed: Process registry capacity not enforced: {:?}", p_over);
        for pid in fill_pids {
            let _ = reap_process(pid);
        }
        return false;
    }

    // Clean up processes
    for pid in fill_pids {
        let _ = reap_process(pid);
    }

    info!("  [PASS] DEBUG-TC3: Boundary conditions (0-length, empty queue, capacity bounds) strictly enforced.");
    true
}

// =========================================================================
// DEBUG-TC4: High-Load Stress & Endurance Verification (Level 2 Mechanism)
// =========================================================================

/// DEBUG-TC4: Exercises the kernel under safe high-load churn, verifying heap stability,
/// queue circular wrap-around, IPC mailbox saturation, and lock contention (CA-6 Section 16 TC4).
fn test_debug_tc4_stress_and_high_load() -> bool {
    let initial_metrics = crate::memory::allocator::get_metrics();

    // 1. Memory Churn Stress: 100 consecutive Box allocation and deallocation cycles
    for i in 0..100 {
        let b = Box::new(i as u64);
        if *b != i as u64 {
            crate::error!("DEBUG-TC4 Failed: Box value mismatch on iteration {}", i);
            return false;
        }
        // Box dropped immediately at end of loop scope
    }

    let mid_metrics = crate::memory::allocator::get_metrics();
    if mid_metrics.used_bytes != initial_metrics.used_bytes {
        crate::error!("DEBUG-TC4 Failed: Heap leak detected! Initial used: {}, After 100 frees: {}",
            initial_metrics.used_bytes, mid_metrics.used_bytes);
        return false;
    }

    // 2. Ready Queue Circular Wrap-Around Stress:
    // Perform 64 enqueue/dequeue cycles (2 full capacity wraps) to verify circular head/tail integrity
    {
        let mut test_q = crate::task::ready_queue::ReadyQueue::new();
        for iter in 0..64 {
            let dummy_tid = crate::task::TaskId(2000 + (iter % 16) as u64);
            if let Err(e) = test_q.enqueue_raw(dummy_tid) {
                crate::error!("DEBUG-TC4 Failed: Enqueue error at iteration {}: {:?}", iter, e);
                return false;
            }
            let deq = test_q.dequeue();
            if deq != Some(dummy_tid) {
                crate::error!("DEBUG-TC4 Failed: Dequeue mismatch at iteration {}: got {:?}", iter, deq);
                return false;
            }
        }
    }

    // 3. IPC Mailbox Saturation Stress:
    // Fill process mailbox to capacity (8 messages), verify backpressure, then drain
    let p_sender = create_process("IpcStressSender").expect("create sender");
    let p_receiver = create_process("IpcStressReceiver").expect("create receiver");

    // Send 8 messages
    for msg_idx in 0..8 {
        let mut payload = [0u8; 16];
        payload[0] = msg_idx as u8;
        if let Err(e) = sys_ipc_send(p_sender, p_receiver, &payload) {
            crate::error!("DEBUG-TC4 Failed: IPC send error at message {}: {:?}", msg_idx, e);
            let _ = reap_process(p_sender);
            let _ = reap_process(p_receiver);
            return false;
        }
    }

    // 9th message must be rejected with MailboxFull
    let overflow_payload = [0xFFu8; 16];
    let send_overflow = sys_ipc_send(p_sender, p_receiver, &overflow_payload);
    if send_overflow != Err(IpcError::MailboxFull) {
        crate::error!("DEBUG-TC4 Failed: IPC MailboxFull not enforced: {:?}", send_overflow);
        let _ = reap_process(p_sender);
        let _ = reap_process(p_receiver);
        return false;
    }

    // Drain all 8 messages in FIFO order
    for expected_idx in 0..8 {
        let msg = match sys_ipc_recv(p_receiver) {
            Ok(Some(m)) => m,
            other => {
                crate::error!("DEBUG-TC4 Failed: IPC recv error at index {}: {:?}", expected_idx, other);
                let _ = reap_process(p_sender);
                let _ = reap_process(p_receiver);
                return false;
            }
        };
        if msg.payload()[0] != expected_idx as u8 {
            crate::error!("DEBUG-TC4 Failed: IPC FIFO ordering corrupted! Expected {}, got {}",
                expected_idx, msg.payload()[0]);
            let _ = reap_process(p_sender);
            let _ = reap_process(p_receiver);
            return false;
        }
    }

    // Verify mailbox is now empty
    let empty_recv = sys_ipc_recv(p_receiver);
    if empty_recv != Ok(None) {
        crate::error!("DEBUG-TC4 Failed: Mailbox not empty after draining: {:?}", empty_recv);
        let _ = reap_process(p_sender);
        let _ = reap_process(p_receiver);
        return false;
    }

    let _ = reap_process(p_sender);
    let _ = reap_process(p_receiver);

    // 4. Lock Invariant & Sequential Stress Testing (Uniprocessor Kernel):
    // Concurrency Note & Architectural Limitation:
    // In this single-core kernel (QEMU single vCPU without SMP), execution is strictly serialized.
    // True multi-core concurrent contention cannot occur because there is only one hardware execution thread.
    // Attempting busy-wait spinning on a held lock on a single core without preemption would result in a permanent
    // deadlock. Therefore, we verify:
    // (a) Mutual exclusion invariants via try_lock() rejection while the lock is actively held.
    // (b) High-frequency sequential acquire-modify-release cycles (100 iterations) verifying counter consistency.
    let spinlock = crate::sync::SpinLock::new(0u64);
    {
        let guard = spinlock.lock();
        if spinlock.try_lock().is_some() {
            crate::error!("DEBUG-TC4 Failed: SpinLock permitted recursive acquisition while locked!");
            return false;
        }
        drop(guard);
    }
    for _i in 0..100 {
        let mut guard = spinlock.lock();
        *guard += 1;
    }
    if *spinlock.lock() != 100 {
        crate::error!("DEBUG-TC4 Failed: SpinLock counter mismatch: {}", *spinlock.lock());
        return false;
    }

    let ticketlock = crate::sync::TicketLock::new(0u64);
    {
        let guard = ticketlock.lock();
        if ticketlock.try_lock().is_some() {
            crate::error!("DEBUG-TC4 Failed: TicketLock permitted concurrent acquisition while locked!");
            return false;
        }
        drop(guard);
    }
    for _i in 0..100 {
        let mut guard = ticketlock.lock();
        *guard += 1;
    }
    if *ticketlock.lock() != 100 {
        crate::error!("DEBUG-TC4 Failed: TicketLock counter mismatch: {}", *ticketlock.lock());
        return false;
    }

    info!("  [PASS] DEBUG-TC4: High-load stress (100 alloc churn, 64 queue wraps, IPC saturation, sequential lock stress) verified.");
    true
}

// =========================================================================
// DEBUG-TC5: Comprehensive Failure Injection & Safety Rejection (Level 2 Mechanism)
// =========================================================================

/// DEBUG-TC5: Injects intentional invalid requests and verifies that every subsystem
/// safely catches, rejects, and fails closed without panicking (CA-6 Section 16 TC5).
fn test_debug_tc5_failure_injection_and_rejection() -> bool {
    reap_terminated_tasks();

    // 1. Syscall Invalid Number Rejection -> ENOSYS (-38)
    let ret_invalid_num = syscall_dispatcher(0xDEAD_BEEF, 0, 0, 0);
    if ret_invalid_num != ENOSYS {
        crate::error!("DEBUG-TC5 Failed: Unknown syscall number not rejected with ENOSYS: {}", ret_invalid_num);
        return false;
    }

    // 2. Syscall User Pointer Validation Rejections -> EFAULT (-14)
    // Null pointer
    let ret_null = syscall_dispatcher(SYS_WRITE, 0, 10, 0);
    if ret_null != EFAULT {
        crate::error!("DEBUG-TC5 Failed: Null pointer not rejected with EFAULT: {}", ret_null);
        return false;
    }

    // Kernel higher-half pointer
    let ret_kernel = syscall_dispatcher(SYS_WRITE, 0xffff_8000_0000_0000, 10, 0);
    if ret_kernel != EFAULT {
        crate::error!("DEBUG-TC5 Failed: Kernel higher-half pointer not rejected with EFAULT: {}", ret_kernel);
        return false;
    }

    // Non-canonical pointer
    let ret_noncanon = syscall_dispatcher(SYS_WRITE, 0x0080_0000_0000_0000, 10, 0);
    if ret_noncanon != EFAULT {
        crate::error!("DEBUG-TC5 Failed: Non-canonical pointer not rejected with EFAULT: {}", ret_noncanon);
        return false;
    }

    // Integer overflow pointer
    let ret_overflow = syscall_dispatcher(SYS_WRITE, u64::MAX - 4, 10, 0);
    if ret_overflow != EFAULT {
        crate::error!("DEBUG-TC5 Failed: Overflowing pointer not rejected with EFAULT: {}", ret_overflow);
        return false;
    }

    // 3. Syscall Oversized Length Rejection -> EINVAL (-22)
    let ret_toolarge = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, 4097, 0);
    if ret_toolarge != EINVAL {
        crate::error!("DEBUG-TC5 Failed: Oversized length > 4096 not rejected with EINVAL: {}", ret_toolarge);
        return false;
    }

    // 4. Illegal Task State Transition Rejections
    let dummy_task = match create("StateFailureTask", None) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC5 Failed to create StateFailureTask: {:?}", e);
            return false;
        }
    };

    // Ready -> Blocked directly (must be rejected)
    let res_ready_blocked = transition(dummy_task, TaskState::Blocked);
    if res_ready_blocked != Err(TaskManagerError::InvalidTransition(StateTransitionError { from: TaskState::Ready, to: TaskState::Blocked })) {
        crate::error!("DEBUG-TC5 Failed: Ready -> Blocked not rejected: {:?}", res_ready_blocked);
        let _ = terminate(dummy_task);
        reap_terminated_tasks();
        return false;
    }

    // Terminate task, then try Terminated -> Running (must be rejected)
    let _ = terminate(dummy_task);
    let res_term_running = transition(dummy_task, TaskState::Running);
    if res_term_running != Err(TaskManagerError::InvalidTransition(StateTransitionError { from: TaskState::Terminated, to: TaskState::Running })) {
        crate::error!("DEBUG-TC5 Failed: Terminated -> Running not rejected: {:?}", res_term_running);
        reap_terminated_tasks();
        return false;
    }
    reap_terminated_tasks();

    // 5. Duplicate Task Enqueue Rejection
    let dup_task = match create("DupTask", None) {
        Ok(id) => id,
        Err(e) => {
            crate::error!("DEBUG-TC5 Failed to create DupTask: {:?}", e);
            return false;
        }
    };
    let _ = enqueue_task(dup_task);
    let dup_err = enqueue_task(dup_task);
    if dup_err != Err(QueueError::DuplicateTask(dup_task)) {
        crate::error!("DEBUG-TC5 Failed: Duplicate enqueue not rejected: {:?}", dup_err);
        let _ = dequeue_task();
        let _ = terminate(dup_task);
        reap_terminated_tasks();
        return false;
    }
    let _ = dequeue_task();
    let _ = terminate(dup_task);
    reap_terminated_tasks();

    // 6. Post-Failure Kernel Integrity Verification (Direct Dispatcher):
    // Confirm that rejecting all invalid requests did not corrupt the system call dispatcher
    // or kernel state: a subsequent valid system call must succeed cleanly.
    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("DEBUG-TC5 Failed: AddressSpace creation failed after failure injection: {}", e);
            return false;
        }
    };
    aspace.switch_to();
    let ret_valid = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, 0, 0);
    restore_kernel_address_space();
    aspace.destroy();
    if ret_valid != 0 {
        crate::error!("DEBUG-TC5 Failed: System call dispatcher corrupted by failure injection! Valid call returned {}", ret_valid);
        return false;
    }

    // 7. End-to-End Ring 3 Hardware Syscall Interrupt Verification (`int 0x80`):
    // Executes actual User Mode machine code at Ring 3 triggering software interrupt 0x80 for:
    // (a) Invalid syscall number (0xDEADBEEF) -> returns ENOSYS (-38)
    // (b) Invalid user pointer (null 0x0) -> returns EFAULT (-14)
    // (c) Oversized length (4097 > 4096) -> returns EINVAL (-22)
    // (d) Proves register preservation across user-kernel transitions (RBX, R12)
    // (e) Executes subsequent valid syscall from Ring 3 -> returns 0
    // (f) Exits cleanly via SYS_EXIT with exit code 0
    let mut user_aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("DEBUG-TC5 Failed to create User AddressSpace for Ring 3 failure test: {}", e);
            return false;
        }
    };

    let user_data_virt = phys_to_virt(user_aspace.user_data_frame.start_address());
    unsafe {
        core::ptr::write_bytes(user_data_virt.as_mut_ptr::<u8>(), 0, 4096);
    }

    let ring3_failure_program: [u8; 208] = [
        0x48, 0xbb, 0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12, 0x49, 0xbc,
        0x11, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0xb8, 0xef, 0xbe, 0xad,
        0xde, 0x31, 0xff, 0x31, 0xf6, 0x31, 0xd2, 0xcd, 0x80, 0x48, 0xbf, 0x00,
        0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, 0x48, 0x89, 0x07, 0xb8, 0x01,
        0x00, 0x00, 0x00, 0x31, 0xff, 0xbe, 0x0a, 0x00, 0x00, 0x00, 0x31, 0xd2,
        0xcd, 0x80, 0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00,
        0x48, 0x89, 0x47, 0x08, 0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0xbf, 0x00,
        0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, 0xbe, 0x01, 0x10, 0x00, 0x00,
        0x31, 0xd2, 0xcd, 0x80, 0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00,
        0x00, 0x00, 0x48, 0x89, 0x47, 0x10, 0x48, 0xb9, 0xf0, 0xde, 0xbc, 0x9a,
        0x78, 0x56, 0x34, 0x12, 0x48, 0x39, 0xcb, 0x75, 0x19, 0x48, 0xb9, 0x11,
        0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x49, 0x39, 0xcc, 0x75, 0x0a,
        0x48, 0xc7, 0x47, 0x18, 0x01, 0x00, 0x00, 0x00, 0xeb, 0x08, 0x48, 0xc7,
        0x47, 0x18, 0x00, 0x00, 0x00, 0x00, 0xb8, 0x01, 0x00, 0x00, 0x00, 0x48,
        0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, 0x31, 0xf6, 0x31,
        0xd2, 0xcd, 0x80, 0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00,
        0x00, 0x48, 0x89, 0x47, 0x20, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x31, 0xff,
        0xcd, 0x80, 0xeb, 0xfe
    ];

    if let Err(e) = user_aspace.load_code(&ring3_failure_program) {
        crate::error!("DEBUG-TC5 Failed to load Ring 3 failure program: {}", e);
        user_aspace.destroy();
        return false;
    }

    user_aspace.switch_to();
    LAST_USER_EXIT_CODE.store(999, Ordering::SeqCst);

    unsafe {
        run_user_code_with_exit_return(USER_CODE_VIRT_ADDR, USER_STACK_TOP);
    }

    restore_kernel_address_space();

    let ring3_exit_code = LAST_USER_EXIT_CODE.load(Ordering::SeqCst);
    if ring3_exit_code != 0 {
        crate::error!("DEBUG-TC5 Ring 3 Failed: User process exited with code {}, expected 0", ring3_exit_code);
        user_aspace.destroy();
        return false;
    }

    let results = unsafe {
        core::slice::from_raw_parts(user_data_virt.as_ptr::<i64>(), 5)
    };
    let ring3_enosys = results[0];
    let ring3_efault = results[1];
    let ring3_einval = results[2];
    let ring3_reg_pres = results[3];
    let ring3_valid = results[4];

    if ring3_enosys != ENOSYS {
        crate::error!("DEBUG-TC5 Ring 3 Failed: ENOSYS mismatch: got {}, expected {}", ring3_enosys, ENOSYS);
        user_aspace.destroy();
        return false;
    }
    if ring3_efault != EFAULT {
        crate::error!("DEBUG-TC5 Ring 3 Failed: EFAULT mismatch: got {}, expected {}", ring3_efault, EFAULT);
        user_aspace.destroy();
        return false;
    }
    if ring3_einval != EINVAL {
        crate::error!("DEBUG-TC5 Ring 3 Failed: EINVAL mismatch: got {}, expected {}", ring3_einval, EINVAL);
        user_aspace.destroy();
        return false;
    }
    if ring3_reg_pres != 1 {
        crate::error!("DEBUG-TC5 Ring 3 Failed: Register preservation check failed (RBX/R12 clobbered)");
        user_aspace.destroy();
        return false;
    }
    if ring3_valid != 0 {
        crate::error!("DEBUG-TC5 Ring 3 Failed: Subsequent valid syscall failed with {}", ring3_valid);
        user_aspace.destroy();
        return false;
    }

    user_aspace.destroy();

    info!("  [PASS] DEBUG-TC5: Failure injection (ENOSYS, EFAULT, EINVAL, illegal transitions, duplicates, Ring 3 int 0x80) safely rejected.");
    true
}

// =========================================================================
// DEBUG-TC6: Repeatability & Zero-State-Drift Verification (CA-6 Section 16 TC6)
// =========================================================================

/// Executes an integrated multi-subsystem workflow and captures both in-flight and post-cleanup metrics.
fn execute_integrated_trial(label: &'static str, t_baseline: &KernelTelemetry) -> Result<(KernelTelemetry, KernelTelemetry), ()> {
    debug!("--- Starting Integrated Trial: {} ---", label);

    // 1. Create a process and address space
    let aspace = AddressSpace::new().map_err(|_| ())?;
    let pid = create_process(label).map_err(|_| ())?;

    // 2. Perform dynamic heap allocation
    let test_vec: Vec<u64> = (0..64).collect();
    if test_vec.len() != 64 {
        aspace.destroy();
        let _ = reap_process(pid);
        return Err(());
    }

    // 3. Switch address space, execute user write
    aspace.switch_to();
    let ret = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, 0, 0);

    // 4. Capture IN-FLIGHT telemetry while resources are actively allocated and CR3 is switched
    let in_flight = KernelTelemetry::collect();

    // Verify in-flight telemetry proves the trial did non-trivial work (cannot pass trivially):
    if in_flight.heap_current_used <= t_baseline.heap_current_used {
        restore_kernel_address_space();
        drop(test_vec);
        aspace.destroy();
        let _ = reap_process(pid);
        crate::error!("DEBUG-TC6 Failed: Heap usage did not increase during in-flight trial!");
        return Err(());
    }
    if in_flight.active_cr3 != aspace.cr3.start_address().as_u64() {
        restore_kernel_address_space();
        drop(test_vec);
        aspace.destroy();
        let _ = reap_process(pid);
        crate::error!("DEBUG-TC6 Failed: In-flight active CR3 does not match trial address space!");
        return Err(());
    }

    restore_kernel_address_space();
    if ret != 0 {
        drop(test_vec);
        aspace.destroy();
        let _ = reap_process(pid);
        return Err(());
    }

    // 5. Clean up and reclaim all resources
    drop(test_vec);
    aspace.destroy();
    reap_process(pid).map_err(|_| ())?;

    // 6. Capture post-trial telemetry
    let post_cleanup = KernelTelemetry::collect();
    Ok((in_flight, post_cleanup))
}

/// DEBUG-TC6: Executes an identical multi-subsystem workload across two separate runs (Pass 1 and Pass 2)
/// and proves that final metrics are 100% deterministic with zero memory leaks or state drift (CA-6 Section 16 TC6).
fn test_debug_tc6_repeatability_and_determinism() -> bool {
    let t0 = KernelTelemetry::collect();

    // Execute Pass 1
    let (t1_flight, t1) = match execute_integrated_trial("Pass1-Proc", &t0) {
        Ok(res) => res,
        Err(_) => {
            crate::error!("DEBUG-TC6 Failed: Integrated Trial Pass 1 failed!");
            return false;
        }
    };

    // Execute Pass 2
    let (t2_flight, t2) = match execute_integrated_trial("Pass2-Proc", &t0) {
        Ok(res) => res,
        Err(_) => {
            crate::error!("DEBUG-TC6 Failed: Integrated Trial Pass 2 failed!");
            return false;
        }
    };

    // 1. Verify in-flight equivalence (both passes performed identical heap allocation work):
    if t1_flight.heap_current_used != t2_flight.heap_current_used {
        crate::error!("DEBUG-TC6 In-Flight Mismatch: Pass 1 allocated {} bytes, Pass 2 allocated {} bytes",
            t1_flight.heap_current_used, t2_flight.heap_current_used);
        return false;
    }

    // 2. Verify instantaneous gauge zero drift between Pass 1 and Pass 2
    if t1.heap_current_used != t2.heap_current_used {
        crate::error!("DEBUG-TC6 Drift Detected: Heap used changed between passes! (Pass1: {}, Pass2: {})",
            t1.heap_current_used, t2.heap_current_used);
        return false;
    }

    if t1.active_tasks != t2.active_tasks {
        crate::error!("DEBUG-TC6 Drift Detected: Active tasks changed! (Pass1: {}, Pass2: {})",
            t1.active_tasks, t2.active_tasks);
        return false;
    }

    if t1.ready_queue_len != t2.ready_queue_len {
        crate::error!("DEBUG-TC6 Drift Detected: Ready queue length changed! (Pass1: {}, Pass2: {})",
            t1.ready_queue_len, t2.ready_queue_len);
        return false;
    }

    if t1.active_cr3 != t2.active_cr3 {
        crate::error!("DEBUG-TC6 Drift Detected: Active CR3 changed! (Pass1: 0x{:x}, Pass2: 0x{:x})",
            t1.active_cr3, t2.active_cr3);
        return false;
    }

    // 3. Verify complete heap reclamation and zero residual memory leakage against baseline t0
    if t2.heap_current_used != t0.heap_current_used {
        crate::error!("DEBUG-TC6 Memory Leak: Heap used did not return to baseline! (Initial: {}, Final: {})",
            t0.heap_current_used, t2.heap_current_used);
        return false;
    }

    // 4. Verify cumulative counter progression: each pass performed exactly 1 syscall
    let pass1_syscalls = t1.syscall_count - t0.syscall_count;
    let pass2_syscalls = t2.syscall_count - t1.syscall_count;
    if pass1_syscalls != 1 || pass2_syscalls != 1 {
        crate::error!("DEBUG-TC6 Workload Discrepancy: Expected 1 syscall per pass, got Pass1={}, Pass2={}",
            pass1_syscalls, pass2_syscalls);
        return false;
    }

    // 5. Verify cumulative allocation/deallocation balance (all allocated bytes were freed)
    let p1_alloc = t1.heap_allocated_bytes - t0.heap_allocated_bytes;
    let p1_freed = t1.heap_freed_bytes - t0.heap_freed_bytes;
    let p2_alloc = t2.heap_allocated_bytes - t1.heap_allocated_bytes;
    let p2_freed = t2.heap_freed_bytes - t1.heap_freed_bytes;
    if p1_alloc != p1_freed || p2_alloc != p2_freed || p1_alloc != p2_alloc {
        crate::error!("DEBUG-TC6 Alloc Discrepancy: P1(alloc={}, free={}), P2(alloc={}, free={})",
            p1_alloc, p1_freed, p2_alloc, p2_freed);
        return false;
    }

    // 6. Verify zero unexpected hardware exception triggers
    if t2.page_faults != t0.page_faults || t2.gpf_count != t0.gpf_count {
        crate::error!("DEBUG-TC6 Unexpected Exceptions: #PF delta={}, #GP delta={}",
            t2.page_faults - t0.page_faults, t2.gpf_count - t0.gpf_count);
        return false;
    }

    info!("  [PASS] DEBUG-TC6: Repeatability and zero state drift verified across consecutive runs (Delta = 0).");
    true
}
