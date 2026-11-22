//! Milestone 12 Automated Verification Tests: Process Management, IPC, Sleep Queue, and Reservations.
//!
//! Conforms to CA-6 Track G (Process/Thread Specialization) and Paper R6.

use crate::{error, info};
use crate::task::process::{
    create_process, reap_process, attach_thread, sys_ipc_send, sys_ipc_recv,
    ProcessState, IpcError, ProcessError, PROCESS_REGISTRY, MAX_THREADS_PER_PROCESS,
    IPC_MAILBOX_CAPACITY,
};
use crate::task::ready_queue::{ReadyQueue, CommitError};
use crate::task::manager::{create, terminate, reap_terminated_tasks, TASK_MANAGER};
use crate::task::task::{TaskId, MAX_ALLOCATABLE_TASK_ID};

/// Runs all programmatic unit and integration tests for Milestone 12.
pub fn run_milestone12_tests() -> bool {
    info!("========================================================");
    info!("    RUNNING MILESTONE 12: PROCESS & IPC TEST SUITE      ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_proc_creation_and_descriptor();
    all_passed &= test_thread_attachment_and_bounds();
    all_passed &= test_generation_persistence_and_stale_rejection();
    all_passed &= test_logical_partition_lifecycle();
    all_passed &= test_ipc_send_receive_fifo();
    all_passed &= test_ipc_stale_sender_rejection();
    all_passed &= test_sleep_queue_wakeups();
    all_passed &= test_queue_reservation_lifecycle();
    all_passed &= test_deferred_task_reclamation();
    all_passed &= test_fail_closed_task_id_allocation();

    if all_passed {
        info!(">>> MILESTONE 12 KERNEL TESTS: ALL 10 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 12 KERNEL TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// PROC-TC1: Process creation, PID generation, and descriptor initialization.
fn test_proc_creation_and_descriptor() -> bool {
    let pid_res = create_process("TestProc-1");
    if let Ok(pid) = pid_res {
        let reg = PROCESS_REGISTRY.lock();
        let desc_res = reg.get_process(pid);
        let ok = desc_res.map_or(false, |d| {
            d.pid == pid && d.name == "TestProc-1" && d.state == ProcessState::Created && d.thread_count == 0
        });
        drop(reg);
        let _ = reap_process(pid);

        if ok {
            info!("  [PASS] PROC-TC1: Process creation, PID ({}), and descriptor verified.", pid);
            return true;
        }
    }
    error!("  [FAIL] PROC-TC1: Process creation or descriptor verification failed!");
    false
}

/// PROC-TC2: Thread attachment to process and thread array bounds (max 4 threads).
fn test_thread_attachment_and_bounds() -> bool {
    let pid = create_process("ThreadProc").expect("create proc");

    let t1 = create("T1", None).unwrap();
    let t2 = create("T2", None).unwrap();
    let t3 = create("T3", None).unwrap();
    let t4 = create("T4", None).unwrap();
    let t5 = create("T5", None).unwrap();

    let a1 = attach_thread(pid, t1).is_ok();
    let a2 = attach_thread(pid, t2).is_ok();
    let a3 = attach_thread(pid, t3).is_ok();
    let a4 = attach_thread(pid, t4).is_ok();
    let a5 = attach_thread(pid, t5); // Should exceed max 4 threads

    let limit_rejected = a5 == Err(ProcessError::ThreadLimitExceeded);

    let reg = PROCESS_REGISTRY.lock();
    let desc = reg.get_process(pid).unwrap();
    let count_ok = desc.thread_count == MAX_THREADS_PER_PROCESS;
    let contains_ok = desc.has_thread(t1) && desc.has_thread(t4);
    drop(reg);

    // Clean up
    terminate(t1).unwrap();
    terminate(t2).unwrap();
    terminate(t3).unwrap();
    terminate(t4).unwrap();
    terminate(t5).unwrap();
    reap_terminated_tasks();
    reap_process(pid).unwrap();

    let passed = a1 && a2 && a3 && a4 && limit_rejected && count_ok && contains_ok;
    if passed {
        info!("  [PASS] PROC-TC2: Thread attachment and capacity bound (4 threads/proc) verified.");
    } else {
        error!("  [FAIL] PROC-TC2: Thread attachment or bounds check failed!");
    }
    passed
}

/// PROC-TC3: Generation persistence across process reap and slot reuse.
fn test_generation_persistence_and_stale_rejection() -> bool {
    let pid1 = create_process("Incarnation-1").expect("create p1");
    let slot_idx = pid1.index;
    let gen1 = pid1.generation;

    // Reap process 1
    reap_process(pid1).expect("reap p1");

    // Old PID1 lookup must now fail
    let old_rejected = PROCESS_REGISTRY.lock().get_process(pid1).is_err();

    // Create process 2: should reuse slot_idx with generation incremented (gen1 + 1)
    let pid2 = create_process("Incarnation-2").expect("create p2");
    let gen2 = pid2.generation;

    let reused_slot = pid2.index == slot_idx;
    let gen_incremented = gen2 == gen1 + 1;

    // Lookups with old PID1 must still fail even though slot is occupied
    let stale_rejected = matches!(PROCESS_REGISTRY.lock().get_process(pid1), Err(ProcessError::StaleProcessId));

    reap_process(pid2).expect("reap p2");

    let passed = old_rejected && reused_slot && gen_incremented && stale_rejected;
    if passed {
        info!("  [PASS] PROC-TC3: Generation persistence (g{} -> g{}) and stale PID rejection verified.", gen1, gen2);
    } else {
        error!("  [FAIL] PROC-TC3: Generation persistence or stale rejection failed!");
    }
    passed
}

/// PROC-TC4: Logical process memory partition allocation (16 KiB heap) and deallocation.
fn test_logical_partition_lifecycle() -> bool {
    let pid = create_process("PartitionProc").expect("create proc");
    let reg = PROCESS_REGISTRY.lock();
    let desc = reg.get_process(pid).unwrap();
    let base = desc.partition_base;
    let size = desc.partition_size;
    let valid_base = base != 0 && (base % 16 == 0);
    let valid_size = size == 16 * 1024;
    drop(reg);

    // Write a test pattern to the allocated logical partition
    unsafe {
        let ptr = base as *mut u8;
        *ptr = 0xDE;
        *ptr.add(size - 1) = 0xAD;
        let test_ok = (*ptr == 0xDE) && (*ptr.add(size - 1) == 0xAD);
        if !test_ok {
            error!("  [FAIL] PROC-TC4: Partition memory read/write test failed!");
            return false;
        }
    }

    reap_process(pid).expect("reap proc");

    let passed = valid_base && valid_size;
    if passed {
        info!("  [PASS] PROC-TC4: Logical 16 KiB heap partition allocation (base: 0x{:x}) verified.", base);
    } else {
        error!("  [FAIL] PROC-TC4: Logical partition allocation failed!");
    }
    passed
}

/// PROC-TC5: IPC message passing (send, receive, FIFO mailbox ordering, and capacity 8).
fn test_ipc_send_receive_fifo() -> bool {
    let p_sender = create_process("IpcSender").expect("create sender");
    let p_recv = create_process("IpcReceiver").expect("create recv");

    // Send 3 messages
    sys_ipc_send(p_sender, p_recv, b"Message-Alpha").expect("send 1");
    sys_ipc_send(p_sender, p_recv, b"Message-Beta").expect("send 2");
    sys_ipc_send(p_sender, p_recv, b"Message-Gamma").expect("send 3");

    // Receive 3 messages in strict FIFO order
    let m1 = sys_ipc_recv(p_recv).unwrap().expect("msg 1");
    let m2 = sys_ipc_recv(p_recv).unwrap().expect("msg 2");
    let m3 = sys_ipc_recv(p_recv).unwrap().expect("msg 3");
    let m4_empty = sys_ipc_recv(p_recv).unwrap().is_none();

    let fifo_ok = m1.payload() == b"Message-Alpha"
        && m2.payload() == b"Message-Beta"
        && m3.payload() == b"Message-Gamma"
        && m1.sender == p_sender;

    // Test mailbox capacity overflow (capacity = 8)
    for _ in 0..IPC_MAILBOX_CAPACITY {
        sys_ipc_send(p_sender, p_recv, b"FloodMsg").expect("flood send");
    }
    let overflow_res = sys_ipc_send(p_sender, p_recv, b"OverflowMsg");
    let overflow_rejected = overflow_res == Err(IpcError::MailboxFull);

    reap_process(p_sender).unwrap();
    reap_process(p_recv).unwrap();

    let passed = fifo_ok && m4_empty && overflow_rejected;
    if passed {
        info!("  [PASS] PROC-TC5: IPC message exchange, strict FIFO order, and mailbox capacity verified.");
    } else {
        error!("  [FAIL] PROC-TC5: IPC send/receive or FIFO check failed!");
    }
    passed
}

/// PROC-TC6: IPC rejection of stale sender messages after sender reap.
fn test_ipc_stale_sender_rejection() -> bool {
    let p_sender = create_process("TempSender").expect("create sender");
    let p_target = create_process("TargetProc").expect("create target");

    // Sender sends message to target
    sys_ipc_send(p_sender, p_target, b"ExpiringNotice").expect("send");

    // Sender exits and is reaped
    reap_process(p_sender).expect("reap sender");

    // Target attempts to receive message: sender generation no longer matches active slot!
    let recv_res = sys_ipc_recv(p_target);
    let stale_rejected = recv_res == Err(IpcError::StaleSender);

    reap_process(p_target).expect("reap target");

    if stale_rejected {
        info!("  [PASS] PROC-TC6: IPC receiver safely rejects messages from reaped stale sender.");
        true
    } else {
        error!("  [FAIL] PROC-TC6: Stale sender IPC rejection failed!");
        false
    }
}

/// PROC-TC7: SleepQueue sleep, tick delta polling, and wakeup of blocked tasks.
fn test_sleep_queue_wakeups() -> bool {
    let mut sq = crate::task::sleep::SleepQueue::new();
    let t1 = TaskId(101);
    let t2 = TaskId(102);

    sq.sleep(t1, 50).expect("sleep t1");
    sq.sleep(t2, 100).expect("sleep t2");

    // At tick 30: no wakeups
    let w30 = sq.poll_wakeups(30);
    let w30_ok = w30[0].is_none() && sq.count == 2;

    // At tick 50: t1 wakes up, t2 remains sleeping
    let w50 = sq.poll_wakeups(50);
    let w50_ok = w50[0] == Some(t1) && w50[1].is_none() && sq.count == 1;

    // At tick 100: t2 wakes up
    let w100 = sq.poll_wakeups(100);
    let w100_ok = w100[0] == Some(t2) && sq.count == 0;

    let passed = w30_ok && w50_ok && w100_ok;
    if passed {
        info!("  [PASS] PROC-TC7: SleepQueue tick delta scheduling and automatic task wakeup verified.");
    } else {
        error!("  [FAIL] PROC-TC7: SleepQueue wakeup polling failed!");
    }
    passed
}

/// PROC-TC8: ReadyQueue two-phase reservation (reserve, commit, cancel, and timeout sweep).
fn test_queue_reservation_lifecycle() -> bool {
    let mut rq = ReadyQueue::new();
    let t1 = TaskId(201);
    let t2 = TaskId(202);

    // 1. Reserve slot at tick 10
    let res1 = rq.reserve(10).expect("reserve 1");
    let res2 = rq.reserve(10).expect("reserve 2");
    let count_reserved = rq.reserved_len() == 2;

    // 2. Commit res1 with t1 at tick 20
    let commit_res = rq.commit_reservation(res1, t1, 20);
    let commit_ok = commit_res.is_ok() && rq.len() == 1 && rq.reserved_len() == 1;

    // 3. Cancel res2
    let cancel_res = rq.cancel_reservation(res2);
    let cancel_ok = cancel_res.is_ok() && rq.len() == 1 && rq.reserved_len() == 0;

    // 4. Test timeout sweep: reserve at tick 0, commit at tick 150 (timeout is 100)
    let res_timeout = rq.reserve(0).expect("reserve timeout token");
    let timeout_commit = rq.commit_reservation(res_timeout, t2, 150);
    let timeout_rejected = match timeout_commit {
        Err((CommitError::ReservationExpired, _)) => true,
        _ => false,
    };
    let capacity_freed = rq.reserved_len() == 0;

    let passed = count_reserved && commit_ok && cancel_ok && timeout_rejected && capacity_freed;
    if passed {
        info!("  [PASS] PROC-TC8: ReadyQueue two-phase reservation (commit, cancel, timeout) verified.");
    } else {
        error!("  [FAIL] PROC-TC8: ReadyQueue reservation lifecycle failed!");
    }
    passed
}

/// PROC-TC9: Deferred task reclamation & stack safety (reap_terminated_tasks cleans Terminated slots).
fn test_deferred_task_reclamation() -> bool {
    // 1. Create a live task (state READY, saved RSP set)
    let t_live = create("LiveTask", None).unwrap();
    let reaped_live = reap_terminated_tasks();
    let live_not_reaped = reaped_live == 0 && TASK_MANAGER.lock().get_task(t_live).is_some();

    // 2. Create tasks to be terminated and reaped
    let t1 = create("ReapMe-1", None).unwrap();
    let t2 = create("ReapMe-2", None).unwrap();

    let init_active = TASK_MANAGER.lock().active_count();

    terminate(t1).unwrap();
    terminate(t2).unwrap();

    let post_term_active = TASK_MANAGER.lock().active_count();
    let reaped = reap_terminated_tasks();

    // 3. Clean up the live task
    terminate(t_live).unwrap();
    let _ = reap_terminated_tasks();

    let passed = live_not_reaped && (init_active >= 2) && (post_term_active == init_active - 2) && (reaped >= 2);
    if passed {
        info!("  [PASS] PROC-TC9: Deferred task reclamation successfully reclaimed {} terminated slots.", reaped);
    } else {
        error!("  [FAIL] PROC-TC9: Deferred task reclamation failed!");
    }
    passed
}

/// PROC-TC10: Fail-closed TaskId allocation.
fn test_fail_closed_task_id_allocation() -> bool {
    let id1 = TaskId::new();
    let id2 = TaskId::new();
    let mono_ok = id2.as_u64() > id1.as_u64();
    let not_max = id2.as_u64() < MAX_ALLOCATABLE_TASK_ID;

    let passed = mono_ok && not_max;
    if passed {
        info!("  [PASS] PROC-TC10: Fail-closed TaskId generation verified ({}, {}).", id1, id2);
    } else {
        error!("  [FAIL] PROC-TC10: TaskId allocation check failed!");
    }
    passed
}
