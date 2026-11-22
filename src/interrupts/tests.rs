//! Milestone 9 interrupt and timer unit tests.

use crate::interrupts::pic::{PIC_1_OFFSET, PIC_2_OFFSET, TIMER_INTERRUPT_ID};
use crate::timer::pit::{PIT_BASE_FREQUENCY_HZ, PIT_DIVISOR, TARGET_FREQUENCY_HZ};
use crate::{error, info};

/// Runs all Milestone 9 interrupt and timer subsystem unit tests.
pub fn run_interrupt_unit_tests() -> bool {
    info!("========================================================");
    info!("   RUNNING MILESTONE 9: INTERRUPT & TIMER UNIT TESTS    ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_pit_divisor_calculation();
    all_passed &= test_pic_configuration();
    all_passed &= test_idt_timer_registration();
    all_passed &= test_timer_tick_progression();
    all_passed &= test_scheduler_preemption_state();
    all_passed &= test_preemption_queue_consistency();

    if all_passed {
        info!(">>> MILESTONE 9 INTERRUPT TESTS: ALL 6 UNIT TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 9 INTERRUPT TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// Test 1: Verifies the 8253/8254 PIT divisor calculation for 100 Hz timer frequency.
pub fn test_pit_divisor_calculation() -> bool {
    let expected_divisor = 11932u16;
    let actual_divisor = PIT_DIVISOR;

    let divisor_ok = actual_divisor == expected_divisor;
    let actual_freq = PIT_BASE_FREQUENCY_HZ as f64 / actual_divisor as f64;
    let diff = (actual_freq - TARGET_FREQUENCY_HZ as f64).abs();
    let error_pct = (diff / TARGET_FREQUENCY_HZ as f64) * 100.0;
    let freq_ok = error_pct < 0.01;

    let pass = divisor_ok && freq_ok;
    if pass {
        info!("  [PASS] INT-TC1: PIT 100 Hz divisor calculation (11932 = 0x2E9C, err < 0.002%) verified.");
    } else {
        error!("  [FAIL] INT-TC1: PIT divisor calculation mismatch!");
    }
    pass
}

/// Test 2: Verifies PIC vector offsets and EOI protocol constants.
pub fn test_pic_configuration() -> bool {
    let pic1_ok = PIC_1_OFFSET == 32;
    let pic2_ok = PIC_2_OFFSET == 40;
    let timer_irq_ok = TIMER_INTERRUPT_ID == 32;

    let pass = pic1_ok && pic2_ok && timer_irq_ok;
    if pass {
        info!("  [PASS] INT-TC2: 8259 PIC remapping (Master: 32, Slave: 40, IRQ0: 32) verified.");
    } else {
        error!("  [FAIL] INT-TC2: PIC vector configuration invalid!");
    }
    pass
}

/// Test 3: Verifies that the IDT entry for vector 32 is populated and marked present.
pub fn test_idt_timer_registration() -> bool {
    let idt_entry = unsafe {
        let idt_ptr = core::ptr::addr_of!(crate::interrupts::idt::IDT);
        let idt_ref: &x86_64::structures::idt::InterruptDescriptorTable = &*idt_ptr;
        &idt_ref[TIMER_INTERRUPT_ID as usize]
    };
    let handler_addr = idt_entry.handler_addr();
    let pass = handler_addr.as_u64() != 0;

    if pass {
        info!("  [PASS] INT-TC3: IDT entry 32 points to valid timer handler at 0x{:x}.", handler_addr.as_u64());
    } else {
        error!("  [FAIL] INT-TC3: IDT timer vector 32 handler not registered!");
    }
    pass
}

/// Test 4: Verifies hardware timer tick progression under PIT interrupts.
pub fn test_timer_tick_progression() -> bool {
    let initial_ticks = crate::timer::get_ticks();
    // Briefly enable interrupts to let PIT timer ticks arrive
    x86_64::instructions::interrupts::enable();
    let mut spins = 0u64;
    while crate::timer::get_ticks() < initial_ticks + 3 && spins < 50_000_000 {
        core::hint::spin_loop();
        spins += 1;
    }
    x86_64::instructions::interrupts::disable();

    let final_ticks = crate::timer::get_ticks();
    let pass = final_ticks >= initial_ticks + 3;
    if pass {
        info!("  [PASS] INT-TC4: Hardware timer tick progression ({} -> {}) verified.", initial_ticks, final_ticks);
    } else {
        error!("  [FAIL] INT-TC4: Hardware timer tick progression failed (initial: {}, final: {})!", initial_ticks, final_ticks);
    }
    pass
}

/// Test 5: Verifies scheduler preemption state transition logic under timer ticks.
pub fn test_scheduler_preemption_state() -> bool {
    use crate::task::manager::TaskManager;
    use crate::task::ready_queue::ReadyQueue;
    use crate::task::state::TaskState;

    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id1 = tm.create_task("P1", None).unwrap();
    let id2 = tm.create_task("P2", None).unwrap();

    // Start with P1 running, P2 in ready queue
    rq.enqueue(id1, &tm).unwrap();
    rq.enqueue(id2, &tm).unwrap();

    let curr = rq.dequeue().unwrap();
    tm.transition_task_sync(curr, TaskState::Running, &mut rq).unwrap();

    // Simulate timer preemption: P1 (running) is preempted, moved to Ready at queue tail
    tm.transition_task_sync(curr, TaskState::Ready, &mut rq).unwrap();
    let next = rq.dequeue().unwrap();
    tm.transition_task_sync(next, TaskState::Running, &mut rq).unwrap();

    let p1_ready = tm.get_task(id1).unwrap().state == TaskState::Ready;
    let p2_running = tm.get_task(id2).unwrap().state == TaskState::Running;
    let p1_in_queue = rq.contains(id1);
    let p2_not_in_queue = !rq.contains(id2);

    let pass = p1_ready && p2_running && p1_in_queue && p2_not_in_queue && next == id2;
    if pass {
        info!("  [PASS] INT-TC5: Scheduler preemption state transition (Running->Ready at tail) verified.");
    } else {
        error!("  [FAIL] INT-TC5: Scheduler preemption state transition failed!");
    }
    pass
}

/// Test 6: Verifies ready queue consistency and exclusion of blocked/terminated tasks under preemption.
pub fn test_preemption_queue_consistency() -> bool {
    use crate::task::manager::TaskManager;
    use crate::task::ready_queue::ReadyQueue;
    use crate::task::state::TaskState;

    let mut tm = TaskManager::new();
    let mut rq = ReadyQueue::new();

    let id_run = tm.create_task("P-Run", None).unwrap();
    let id_block = tm.create_task("P-Block", None).unwrap();
    let id_term = tm.create_task("P-Term", None).unwrap();

    rq.enqueue(id_run, &tm).unwrap();
    rq.enqueue(id_block, &tm).unwrap();
    rq.enqueue(id_term, &tm).unwrap();

    // P-Block transitions to Blocked
    let b = rq.dequeue().unwrap(); // id_run
    tm.transition_task_sync(b, TaskState::Running, &mut rq).unwrap();
    let b2 = rq.dequeue().unwrap(); // id_block
    tm.transition_task_sync(b2, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(b2, TaskState::Blocked, &mut rq).unwrap();

    // P-Term transitions to Terminated
    let b3 = rq.dequeue().unwrap(); // id_term
    tm.transition_task_sync(b3, TaskState::Running, &mut rq).unwrap();
    tm.transition_task_sync(b3, TaskState::Terminated, &mut rq).unwrap();

    // Under timer tick, only Ready tasks can be queued
    let block_in_q = rq.contains(id_block);
    let term_in_q = rq.contains(id_term);
    let pass = !block_in_q && !term_in_q && rq.is_empty();

    if pass {
        info!("  [PASS] INT-TC6: Ready queue consistency under preemption (blocked/term excluded) verified.");
    } else {
        error!("  [FAIL] INT-TC6: Ready queue consistency check failed!");
    }
    pass
}
