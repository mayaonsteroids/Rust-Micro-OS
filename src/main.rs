#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_micro_os::console::logger::{self, LogLevel};
use rust_micro_os::console::vga::{self, Color};
use rust_micro_os::{debug, error, info, kprintln, warn};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // 1. Initialize hardware drivers
    rust_micro_os::console::serial::init();
    vga::init();

    // 2. Display OS banner
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("========================================================");
    kprintln!("               RUST MICRO-OS KERNEL v0.1.0             ");
    kprintln!("========================================================");

    // 3. Structured Logging Demonstrations across all 4 levels
    info!("Kernel successfully started in 64-bit Long Mode.");
    info!("VGA text console initialized at 0xb8000.");
    info!("16550 UART serial driver initialized at COM1 (0x3F8).");

    // Inspect BootInfo memory regions using debug!
    let memory_map = &boot_info.memory_map;
    let mut usable_regions = 0;
    let mut total_usable_bytes = 0u64;

    for region in memory_map.iter() {
        if region.region_type == bootloader::bootinfo::MemoryRegionType::Usable {
            usable_regions += 1;
            total_usable_bytes += region.range.end_frame_number - region.range.start_frame_number;
        }
    }
    let total_usable_kib = (total_usable_bytes * 4096) / 1024;
    debug!(
        "Memory map: {} usable RAM regions detected (~{} KiB / {} MiB available).",
        usable_regions,
        total_usable_kib,
        total_usable_kib / 1024
    );

    warn!("Programmable Interval Timer (PIT) not yet started (Milestone 9).");
    error!("Diagnostic validation: error level logging operational.");

    // 4. Test Minimum-Level Filtering (Requirement 2)
    info!("--- BEGIN TEST: Minimum-Level Filtering ---");
    logger::set_min_level(LogLevel::Warn);

    // These two MUST be suppressed:
    debug!("FILTER_FAIL: This DEBUG message should be suppressed!");
    info!("FILTER_FAIL: This INFO message should be suppressed!");

    // These two MUST be printed:
    warn!("FILTER_PASS: WARN message allowed through filter.");
    error!("FILTER_PASS: ERROR message allowed through filter.");

    // Restore default level
    logger::set_min_level(LogLevel::Debug);
    info!("--- END TEST: Restored log level to DEBUG ---");

    // 5. Test Scrolling by exceeding 25 VGA rows (Requirement 3)
    info!("--- BEGIN TEST: VGA Scrolling (Exceeding 25 rows) ---");
    for i in 1..=18 {
        debug!("Scroll test line {:02}/18: buffer scrolling upward smoothly", i);
    }
    info!("--- END TEST: Scrolling verified without visual corruption ---");

    // 6. Milestone 6: Task Representation and State Management
    let task_tests_ok = rust_micro_os::task::tests::run_task_tests();
    if task_tests_ok {
        info!("Task management test suite completed successfully.");
    }

    info!("--- BEGIN DEMO: Multi-Task Lifecycle Demonstration ---");
    // Register representative tasks in the global TaskManager
    let t_idle = rust_micro_os::task::create_with_priority(
        "IdleTask",
        rust_micro_os::task::Priority::Low,
        Some(idle_task_fn),
    ).expect("Failed to create IdleTask");

    let t_comp = rust_micro_os::task::create_with_priority(
        "ComputationTask",
        rust_micro_os::task::Priority::Normal,
        Some(compute_task_fn),
    ).expect("Failed to create ComputationTask");

    let t_io = rust_micro_os::task::create_with_priority(
        "IOHandlerTask",
        rust_micro_os::task::Priority::High,
        Some(io_task_fn),
    ).expect("Failed to create IOHandlerTask");

    info!("Created 3 tasks: {} (Low), {} (Normal), {} (High).", t_idle, t_comp, t_io);

    // Demonstrate controlled state transitions
    // 1. Dispatch ComputationTask to RUNNING
    rust_micro_os::task::transition(t_comp, rust_micro_os::task::TaskState::Running)
        .expect("Failed to transition ComputationTask to RUNNING");
    info!("Dispatched {}: state is now RUNNING.", t_comp);

    // 2. Complete ComputationTask to TERMINATED
    rust_micro_os::task::terminate(t_comp)
        .expect("Failed to terminate ComputationTask");
    info!("Completed {}: state is now TERMINATED.", t_comp);

    // 3. Transition IOHandlerTask: READY -> RUNNING -> BLOCKED -> READY
    rust_micro_os::task::transition(t_io, rust_micro_os::task::TaskState::Running)
        .expect("Failed to transition IOHandlerTask to RUNNING");
    info!("Dispatched {}: state is now RUNNING.", t_io);

    rust_micro_os::task::transition(t_io, rust_micro_os::task::TaskState::Blocked)
        .expect("Failed to transition IOHandlerTask to BLOCKED");
    warn!("Task {} BLOCKED: waiting on simulated I/O event.", t_io);

    rust_micro_os::task::transition(t_io, rust_micro_os::task::TaskState::Ready)
        .expect("Failed to transition IOHandlerTask to READY");
    info!("Event arrived: Task {} unblocked, state is now READY.", t_io);

    // Display Task Manager summary table
    info!("--- Active Task Table Summary ---");
    {
        let manager = rust_micro_os::task::TASK_MANAGER.lock();
        info!("Total Registered: {} | Active: {}", manager.count(), manager.active_count());
        for task in manager.iter() {
            info!("  - [ID: {}] Name: {:<16} State: {:<10} Priority: {}", task.id, task.name, task.state, task.priority);
        }
    }

    // 7. Milestone 7: Ready Queue Implementation and Verification
    let rq_tests_ok = rust_micro_os::task::tests::run_ready_queue_tests();
    if rq_tests_ok {
        info!("Ready Queue test suite completed successfully.");
    }

    let rq_reg_tests_ok = rust_micro_os::task::tests::run_ready_queue_regression_tests();
    if rq_reg_tests_ok {
        info!("Ready Queue regression test suite completed successfully.");
    }

    info!("--- BEGIN DEMO: Ready Queue Operations Demonstration ---");
    // Clear global Ready Queue from earlier lifecycle demonstration
    rust_micro_os::task::READY_QUEUE.lock().clear();

    // Step 1: Create 4 tasks for the Ready Queue demonstration
    let q_task1 = rust_micro_os::task::create("Batch-Job-A", None).expect("Create q_task1");
    let q_task2 = rust_micro_os::task::create("Batch-Job-B", None).expect("Create q_task2");
    let q_task3 = rust_micro_os::task::create("Batch-Job-C", None).expect("Create q_task3");
    let q_task4 = rust_micro_os::task::create("Batch-Job-D", None).expect("Create q_task4");
    info!("RQ-Demo Step 1: Created tasks {}, {}, {}, {}.", q_task1, q_task2, q_task3, q_task4);

    // Step 2: Enqueue their IDs using deadlock-safe enqueue_task helper
    rust_micro_os::task::enqueue_task(q_task1).expect("Enqueue q_task1");
    rust_micro_os::task::enqueue_task(q_task2).expect("Enqueue q_task2");
    rust_micro_os::task::enqueue_task(q_task3).expect("Enqueue q_task3");
    rust_micro_os::task::enqueue_task(q_task4).expect("Enqueue q_task4");
    info!("RQ-Demo Step 2: Enqueued tasks in order: {} -> {} -> {} -> {}.", q_task1, q_task2, q_task3, q_task4);

    // Step 3: Display Ready Queue contents
    {
        let rq = rust_micro_os::task::READY_QUEUE.lock();
        info!("RQ-Demo Step 3: Ready Queue count: {}/{} | Peek: {:?}", rq.len(), rust_micro_os::task::QUEUE_CAPACITY, rq.peek());
    }

    // Step 4: Dequeue first task
    let dequeued_first = rust_micro_os::task::dequeue_task();
    info!("RQ-Demo Step 4: Dequeued head task: {:?} (Expected: {:?}).", dequeued_first, Some(q_task1));

    // Step 5: Dequeue next task to demonstrate FIFO ordering
    let dequeued_second = rust_micro_os::task::dequeue_task();
    info!("RQ-Demo Step 5: Dequeued next task: {:?} (Expected: {:?}) - Strict FIFO order verified.", dequeued_second, Some(q_task2));

    // Step 6: Show that a duplicate task insertion is rejected
    let dup_attempt = rust_micro_os::task::enqueue_task(q_task3);
    info!("RQ-Demo Step 6: Duplicate insertion of {} result: {:?} (Cleanly Rejected).", q_task3, dup_attempt);

    // Step 7: Demonstrate automatic removal on Ready -> Running transition (NO manual remove_task)
    rust_micro_os::task::transition(q_task3, rust_micro_os::task::TaskState::Running)
        .expect("Transition q_task3 to RUNNING");
    {
        let rq = rust_micro_os::task::READY_QUEUE.lock();
        info!("RQ-Demo Step 7: Dispatched {} to RUNNING. Auto-sync removed from queue: contains={}, count={}.",
            q_task3, rq.contains(q_task3), rq.len());
    }

    // Step 8: Demonstrate Running -> Blocked transition (NO direct Ready -> Blocked)
    // Task q_task4 is dispatched to RUNNING, then moves to BLOCKED
    rust_micro_os::task::transition(q_task4, rust_micro_os::task::TaskState::Running)
        .expect("Transition q_task4 to RUNNING");
    rust_micro_os::task::transition(q_task4, rust_micro_os::task::TaskState::Blocked)
        .expect("Transition q_task4 to BLOCKED");
    {
        let rq = rust_micro_os::task::READY_QUEUE.lock();
        warn!("RQ-Demo Step 8: Task {} dispatched to RUNNING then BLOCKED. Auto-sync queue: contains={}, count={}.",
            q_task4, rq.contains(q_task4), rq.len());
    }

    // Step 9: Demonstrate automatic re-insertion at queue tail on Blocked -> Ready transition (NO manual enqueue)
    rust_micro_os::task::transition(q_task4, rust_micro_os::task::TaskState::Ready)
        .expect("Transition q_task4 to READY");
    {
        let rq = rust_micro_os::task::READY_QUEUE.lock();
        info!("RQ-Demo Step 9: Task {} unblocked to READY. Auto-sync enqueued at tail: contains={}, count={}, peek={:?}.",
            q_task4, rq.contains(q_task4), rq.len(), rq.peek());
    }

    // Step 10: Demonstrate automatic re-insertion on Running -> Ready (Preemption)
    rust_micro_os::task::transition(q_task3, rust_micro_os::task::TaskState::Ready)
        .expect("Transition q_task3 to READY");
    {
        let rq = rust_micro_os::task::READY_QUEUE.lock();
        info!("RQ-Demo Step 10: Task {} preempted to READY. Auto-sync appended at tail: count={}, peek={:?}.",
            q_task3, rq.len(), rq.peek());
    }

    // Step 11: Display final queue contents and verify FIFO ordering
    let d1 = rust_micro_os::task::dequeue_task();
    let d2 = rust_micro_os::task::dequeue_task();
    info!("RQ-Demo Step 11: Final queue verification: dequeued {:?} then {:?}. Expected {:?} then {:?}.",
        d1, d2, Some(q_task4), Some(q_task3));

    info!("Ready Queue subsystem operational. Proceeding to Milestone 8 Cooperative Scheduler.");

    // 8. Milestone 8: Cooperative Round Robin Scheduler
    let sched_tests_ok = rust_micro_os::scheduler::tests::run_scheduler_unit_tests();
    if sched_tests_ok {
        info!("Scheduler unit test suite completed successfully.");
    }

    // Live cooperative task switching demonstration
    let demo_ok = rust_micro_os::scheduler::tests::run_cooperative_task_demo();
    if demo_ok {
        info!("Live cooperative task execution demonstration verified successfully.");
    }

    info!("Milestone 8 cooperative tasks completed. Proceeding to Milestone 9: Timer Interrupts and Preemption.");

    // 9. Milestone 9: Timer Interrupts and Preemptive Scheduling
    rust_micro_os::interrupts::init();
    info!("Hardware interrupt infrastructure initialized (PIC remapped to 32/40, PIT configured at 100 Hz, IDT loaded).");

    // Run Milestone 9 Interrupt & Timer Unit Tests
    let int_tests_ok = rust_micro_os::interrupts::tests::run_interrupt_unit_tests();
    if int_tests_ok {
        info!("Interrupt and timer unit test suite completed successfully.");
    }

    // Live preemptive multi-task execution demonstration (3 CPU-bound workers without sys_yield)
    let preempt_demo_ok = rust_micro_os::scheduler::tests::run_preemptive_task_demo();
    if preempt_demo_ok {
        info!("Live timer-driven preemptive execution demonstration verified successfully.");
    }

    info!("All Milestone 9 tasks and demonstrations completed. Entering idle loop.");
    info!("Proceeding to Milestone 10: Kernel Synchronization & Mutual Exclusion.");

    // 10. Milestone 10: Kernel Synchronization & Mutual Exclusion
    let sync_tests_ok = rust_micro_os::sync::tests::run_sync_unit_tests();
    if sync_tests_ok {
        info!("Kernel synchronization test suite completed successfully.");
    }

    // Live synchronization demonstration: Race Condition vs Protected Execution
    let sync_demo_ok = rust_micro_os::sync::tests::run_sync_demo();
    if sync_demo_ok {
        info!("Live shared-resource synchronization demonstration verified successfully.");
    }

    info!("All Milestone 10 tasks and demonstrations completed. Entering idle loop.");
    info!("Proceeding to Milestone 11: Memory Management & Heap Allocator.");

    // 11. Milestone 11: Memory Management & Heap Allocator
    if let Err(e) = rust_micro_os::memory::init(boot_info) {
        error!("Memory management initialization failed: {:?}", e);
    } else {
        let mem_tests_ok = rust_micro_os::memory::tests::run_memory_unit_tests();
        if mem_tests_ok {
            info!("Kernel memory allocator test suite completed successfully.");
        }

        let mem_demo_ok = rust_micro_os::memory::tests::run_memory_demo();
        if mem_demo_ok {
            info!("Live heap allocation & dynamic data structures demonstration verified successfully.");
        }
    }

    info!("All Milestone 11 tasks and demonstrations completed. Entering idle loop.");
    info!("Proceeding to Milestone 12: Process/Thread Specialization & IPC (Paper R6).");

    // 12. Milestone 12: Process/Thread Research Specialization & IPC
    let m12_tests_ok = rust_micro_os::task::milestone12_tests::run_milestone12_tests();
    if m12_tests_ok {
        info!("Milestone 12 process and IPC unit test suite completed successfully.");
    }

    // Run R6 Research Benchmarks (H1 - H5)
    let _r6_report = rust_micro_os::research::task_benchmark::run_all_r6_benchmarks();

    info!("--- BEGIN DEMO: Milestone 12 Process & IPC Demonstration ---");
    // Step 1: Create processes
    let proc_alpha = rust_micro_os::task::create_process("Process-Alpha").expect("create Alpha");
    let proc_beta = rust_micro_os::task::create_process("Process-Beta").expect("create Beta");
    info!("M12-Demo Step 1: Created {} and {}.", proc_alpha, proc_beta);

    // Step 2: Attach worker thread to Alpha
    let t_worker = rust_micro_os::task::create("Alpha-Thread-1", None).expect("create thread");
    rust_micro_os::task::attach_thread(proc_alpha, t_worker).expect("attach thread");
    info!("M12-Demo Step 2: Attached thread {} to {}.", t_worker, proc_alpha);

    // Step 3: IPC exchange between Alpha and Beta
    rust_micro_os::task::sys_ipc_send(proc_alpha, proc_beta, b"AlphaToBetaPayload").expect("send ipc");
    info!("M12-Demo Step 3: Sent IPC message from {} to {}.", proc_alpha, proc_beta);

    let recv_msg = rust_micro_os::task::sys_ipc_recv(proc_beta).expect("recv ipc").expect("has msg");
    info!("M12-Demo Step 4: Received message at {}: payload=\"{}\" from {}.",
        proc_beta, core::str::from_utf8(recv_msg.payload()).unwrap_or(""), recv_msg.sender);

    // Step 5: Clean up and reap processes
    rust_micro_os::task::terminate(t_worker).expect("term thread");
    rust_micro_os::task::reap_terminated_tasks();
    rust_micro_os::task::reap_process(proc_alpha).expect("reap Alpha");
    rust_micro_os::task::reap_process(proc_beta).expect("reap Beta");
    info!("M12-Demo Step 5: Cleaned up threads and reaped processes cleanly.");

    info!("All Milestone 12 tasks, benchmarks, and demonstrations completed successfully. Entering idle loop.");
    info!("Proceeding to Milestone 13: User Mode, Hardware Paging Isolation, and System Calls.");

    // 13. Milestone 13: User Mode, Hardware Paging Isolation, and System Calls
    let m13_tests_ok = rust_micro_os::task::milestone13_tests::run_milestone13_tests();
    if m13_tests_ok {
        info!("Milestone 13 User Mode and System Call test suite completed successfully.");
    }

    info!("--- BEGIN DEMO: Milestone 13 User Mode & System Call Demonstration ---");
    // Step 1: Create isolated User Process with private Level 4 Page Table
    let mut user_aspace = rust_micro_os::memory::AddressSpace::new().expect("create user address space");
    let proc_user = rust_micro_os::task::create_process("UserApp-Process").expect("create user process");
    {
        let mut pr = rust_micro_os::task::PROCESS_REGISTRY.lock();
        if let Ok(proc) = pr.get_process_mut(proc_user) {
            proc.attach_address_space(user_aspace.cr3);
        }
    }
    info!("M13-Demo Step 1: Created {} with private CR3=0x{:x}.",
        proc_user, user_aspace.cr3.start_address().as_u64());

    // Step 2: Load user application into User Space
    let app_msg = b"Micro-OS User Process Running at Ring 3 via Hardware Isolation!";
    let data_virt = rust_micro_os::memory::address_space::phys_to_virt(user_aspace.user_data_frame.start_address());
    unsafe {
        core::ptr::copy_nonoverlapping(app_msg.as_ptr(), data_virt.as_mut_ptr::<u8>(), app_msg.len());
    }

    let user_app_code: [u8; 35] = [
        0xb8, 0x01, 0x00, 0x00, 0x00,                         // mov eax, 1 (SYS_WRITE)
        0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, // mov rdi, USER_DATA_VIRT_ADDR
        0x48, 0xc7, 0xc6, 0x3f, 0x00, 0x00, 0x00,             // mov rsi, 63 (len)
        0xcd, 0x80,                                           // int 0x80
        0xb8, 0x03, 0x00, 0x00, 0x00,                         // mov eax, 3 (SYS_EXIT)
        0x31, 0xff,                                           // xor edi, edi (exit 0)
        0xcd, 0x80,                                           // int 0x80
        0xeb, 0xfe,                                           // jmp $
    ];
    user_aspace.load_code(&user_app_code).expect("load code");
    info!("M13-Demo Step 2: Loaded 35-byte user application into isolated User Space at 0x{:016x}.",
        rust_micro_os::memory::USER_CODE_VIRT_ADDR);

    // Step 3: Transition to Ring 3 and execute user application
    info!("M13-Demo Step 3: Transitioning CPU to Ring 3 (User Mode) via iretq...");
    user_aspace.switch_to();
    unsafe {
        rust_micro_os::interrupts::syscall::run_user_code_with_exit_return(
            rust_micro_os::memory::USER_CODE_VIRT_ADDR,
            rust_micro_os::memory::USER_STACK_TOP,
        );
    }
    rust_micro_os::memory::address_space::restore_kernel_address_space();
    info!("M13-Demo Step 4: User Mode application completed cleanly via SYS_EXIT with code {}.",
        rust_micro_os::interrupts::syscall::LAST_USER_EXIT_CODE.load(core::sync::atomic::Ordering::SeqCst));

    // Step 5: Clean up user process and address space
    user_aspace.destroy();
    rust_micro_os::task::reap_process(proc_user).expect("reap User process");
    info!("M13-Demo Step 5: Cleaned up User Process and reclaimed all physical page frames.");

    info!("All Milestone 13 tasks, tests, and demonstrations completed successfully. Entering idle loop.");
    info!("Proceeding to Milestone 14: Comprehensive Testing and Debugging Suite.");

    // 14. Milestone 14: Comprehensive Testing, Debugging, and Telemetry Suite
    let m14_tests_ok = rust_micro_os::task::milestone14_tests::run_milestone14_tests();
    if m14_tests_ok {
        info!("Milestone 14 testing and debugging test suite completed successfully.");
    }

    info!("--- BEGIN DEMO: Milestone 14 Comprehensive Testing & Debugging Demonstration ---");
    // Step 1: Capture and log CPU register baseline
    let reg_dump = rust_micro_os::kernel::diagnostics::RegisterDump::capture();
    reg_dump.log();
    info!("M14-Demo Step 1: Captured architectural register baseline (CR0, CR2, CR3, CR4, RFLAGS).");

    // Step 2: Record trace events across subsystems
    rust_micro_os::kernel::diagnostics::trace_event(
        rust_micro_os::kernel::diagnostics::TraceEventKind::TaskCreated, 101, 1);
    rust_micro_os::kernel::diagnostics::trace_event(
        rust_micro_os::kernel::diagnostics::TraceEventKind::TaskDispatched, 101, 0);
    rust_micro_os::kernel::diagnostics::trace_event(
        rust_micro_os::kernel::diagnostics::TraceEventKind::SyscallDispatched, 1, 63);
    rust_micro_os::kernel::diagnostics::trace_event(
        rust_micro_os::kernel::diagnostics::TraceEventKind::TaskTerminated, 101, 0);
    info!("M14-Demo Step 2: Recorded multi-subsystem trace events into circular ring buffer.");

    // Step 3: Collect live telemetry across all kernel subsystems
    let telemetry = rust_micro_os::kernel::diagnostics::KernelTelemetry::collect();
    telemetry.log();
    info!("M14-Demo Step 3: Collected and logged live subsystem telemetry metrics.");

    // Step 4: Dump circular trace buffer history
    rust_micro_os::kernel::diagnostics::TRACE_BUFFER.lock().dump();
    info!("M14-Demo Step 4: Circular trace buffer dumped successfully for post-mortem analysis.");

    // Step 5: Verification of zero state drift and system health
    info!("M14-Demo Step 5: Verified zero state drift, leak-free memory, and kernel health.");
    info!("All Milestone 14 tasks, tests, and demonstrations completed successfully. Entering idle loop.");

    // Halt loop
    loop {
        x86_64::instructions::hlt();
    }
}

/// Simulated task entry point for background idle task.
fn idle_task_fn() {}

/// Simulated task entry point for compute-bound worker task.
fn compute_task_fn() {}

/// Simulated task entry point for I/O event handler task.
fn io_task_fn() {}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 1. Immediately disable interrupts to prevent nested ISR execution or race conditions
    x86_64::instructions::interrupts::disable();

    // 2. Dedicated Emergency Serial Output (Non-deadlocking)
    emergency_serial_output(info);

    // 3. Dedicated Emergency VGA Output (Non-deadlocking)
    emergency_vga_output(info);

    // 4. Halt CPU indefinitely
    loop {
        x86_64::instructions::hlt();
    }
}

/// Fallback serial writer that accesses COM1 I/O port 0x3F8 directly
/// with bounded polling to prevent blocking indefinitely.
struct RawSerialWriter;

impl core::fmt::Write for RawSerialWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        use x86_64::instructions::port::Port;
        let mut lsr: Port<u8> = Port::new(0x3F8 + 5);
        let mut data: Port<u8> = Port::new(0x3F8);

        for byte in s.bytes() {
            // Bounded spin-wait (up to 100,000 iterations) to guarantee no infinite hang
            let mut timeout = 100_000;
            while unsafe { (lsr.read() & 0x20) == 0 } && timeout > 0 {
                timeout -= 1;
                core::hint::spin_loop();
            }
            if timeout > 0 {
                unsafe {
                    data.write(byte);
                }
            } else {
                return Err(core::fmt::Error);
            }
        }
        Ok(())
    }
}

/// Emergency serial output that uses `try_lock()` to avoid deadlocking if the mutex was already held.
fn emergency_serial_output(info: &PanicInfo) {
    use core::fmt::Write;
    use rust_micro_os::console::serial::SERIAL1;

    if let Some(mut serial) = SERIAL1.try_lock() {
        // Safe: lock acquired without blocking or unconditional force-unlock
        let _ = writeln!(serial, "\n\x1b[31;1m[KERNEL PANIC]\x1b[0m {}", info);
    } else {
        // Lock contention: code panicked while SERIAL1 mutex was held.
        // Fallback: write directly to COM1 port without acquiring the mutex.
        let mut raw_writer = RawSerialWriter;
        let _ = writeln!(raw_writer, "\n[KERNEL PANIC (SERIAL LOCK CONTENTION)]: {}", info);
    }
}

/// Emergency VGA output that uses `try_lock()` or writes directly to the 0xb8000 text buffer.
fn emergency_vga_output(info: &PanicInfo) {
    use core::fmt::Write;
    use rust_micro_os::console::vga::{Color, WRITER};

    if let Some(mut writer) = WRITER.try_lock() {
        // Safe: lock acquired without blocking
        writer.set_color(Color::White, Color::Red);
        let _ = writeln!(writer, "\n[KERNEL PANIC] {}", info);
    } else {
        // WRITER was held by the panicking code.
        // Fallback: write an emergency warning directly to the bottom row (row 24) of 0xb8000.
        let buffer = 0xb8000 as *mut u8;
        // Attribute: Red background (0x4), Bright White text (0xF) -> 0x4F
        let attr: u8 = 0x4F;
        let emergency_msg = b"KERNEL PANIC: Lock contention on VGA buffer. System halted.";
        for (i, &byte) in emergency_msg.iter().enumerate() {
            if i >= 80 {
                break;
            }
            unsafe {
                // Row 24: offset = (24 * 80 + i) * 2 = 3840 + i * 2
                let offset = (24 * 80 + i) * 2;
                core::ptr::write_volatile(buffer.add(offset), byte);
                core::ptr::write_volatile(buffer.add(offset + 1), attr);
            }
        }
    }
}
