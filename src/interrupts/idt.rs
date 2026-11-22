//! Interrupt Descriptor Table (IDT) configuration, CPU exception handlers,
//! and Milestone 13 System Call interrupt gate (Vector 0x80).

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::VirtAddr;
use crate::interrupts::pic::TIMER_INTERRUPT_ID;
use crate::{error, warn};

/// Global Interrupt Descriptor Table instance.
pub static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

/// Atomic flags and diagnostic counters for User Mode exception testing:
pub static EXPECTED_USER_FAULT: AtomicBool = AtomicBool::new(false);
pub static EXPECTED_FAULT_ADDR: AtomicU64 = AtomicU64::new(0);
pub static EXPECTED_FAULT_RIP: AtomicU64 = AtomicU64::new(0);
pub static USER_FAULT_COUNT: AtomicU64 = AtomicU64::new(0);
pub static LAST_FAULT_ADDR: AtomicU64 = AtomicU64::new(0);
pub static LAST_FAULT_RIP: AtomicU64 = AtomicU64::new(0);
pub static LAST_FAULT_ERROR_CODE: AtomicU64 = AtomicU64::new(0);
pub static FAULT_RECOVERY_RIP: AtomicU64 = AtomicU64::new(0);

pub static EXPECTED_USER_GPF: AtomicBool = AtomicBool::new(false);
pub static USER_GPF_COUNT: AtomicU64 = AtomicU64::new(0);
pub static GPF_RECOVERY_RIP: AtomicU64 = AtomicU64::new(0);

/// Initializes and loads the IDT with exception handlers, timer IRQ, and syscall gate (0x80).
pub fn init() {
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);

        // Register core architectural exception handlers
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        // Register naked assembly timer interrupt entry at vector 32 (IRQ 0)
        let timer_addr = VirtAddr::new(crate::scheduler::context::timer_interrupt_entry as *const () as usize as u64);
        idt[TIMER_INTERRUPT_ID as usize].set_handler_addr(timer_addr);

        // Register naked assembly syscall entry at vector 0x80 with Ring 3 privilege
        let syscall_addr = VirtAddr::new(crate::interrupts::syscall::syscall_interrupt_entry as *const () as usize as u64);
        idt[0x80].set_handler_addr(syscall_addr).set_privilege_level(x86_64::PrivilegeLevel::Ring3);

        idt.load();
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    warn!("[CPU EXCEPTION: BREAKPOINT]\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    error!("[FATAL CPU EXCEPTION: DOUBLE FAULT]\n{:#?}", stack_frame);
    loop {
        x86_64::instructions::hlt();
    }
}

/// Evaluates whether an incoming page fault qualifies for safe test recovery.
///
/// Invariants enforced:
/// - One-shot recovery flag (`EXPECTED_USER_FAULT`) must be armed
/// - `expected_addr` must be non-zero (unset address 0 never authorizes recovery)
/// - `fault_addr` must strictly equal `expected_addr`
/// - `expected_rip` must be non-zero (unset RIP 0 never authorizes recovery)
/// - `fault_rip` must strictly equal `expected_rip`
/// - Execution origin must be User Mode Ring 3 (`code_segment == 0x23`)
/// - Error code must strictly equal 0x5 (present-page protection violation from user mode, bit 0 = 1, bit 2 = 1, all others 0)
pub fn can_recover_page_fault(
    code_segment: u64,
    error_code: PageFaultErrorCode,
    fault_addr: u64,
    fault_rip: u64,
) -> bool {
    let is_user_cs = code_segment == 0x23;
    let is_exact_err = error_code.bits() == 0x5;
    let is_armed = EXPECTED_USER_FAULT.load(Ordering::SeqCst);
    let expected_addr = EXPECTED_FAULT_ADDR.load(Ordering::SeqCst);
    let expected_rip = EXPECTED_FAULT_RIP.load(Ordering::SeqCst);

    is_user_cs
        && is_exact_err
        && is_armed
        && expected_addr != 0
        && fault_addr == expected_addr
        && expected_rip != 0
        && fault_rip == expected_rip
}

extern "x86-interrupt" fn page_fault_handler(
    mut stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    let fault_addr = Cr2::read();
    let fault_rip = stack_frame.instruction_pointer.as_u64();

    if can_recover_page_fault(
        stack_frame.code_segment,
        error_code,
        fault_addr.as_u64(),
        fault_rip,
    ) {
        // Atomically consume the one-shot recovery flag: exactly once!
        if EXPECTED_USER_FAULT.swap(false, Ordering::SeqCst) {
            EXPECTED_FAULT_ADDR.store(0, Ordering::SeqCst);
            EXPECTED_FAULT_RIP.store(0, Ordering::SeqCst);
            let recovery_rip = FAULT_RECOVERY_RIP.swap(0, Ordering::SeqCst);
            USER_FAULT_COUNT.fetch_add(1, Ordering::SeqCst);
            LAST_FAULT_ADDR.store(fault_addr.as_u64(), Ordering::SeqCst);
            LAST_FAULT_RIP.store(fault_rip, Ordering::SeqCst);
            LAST_FAULT_ERROR_CODE.store(error_code.bits(), Ordering::SeqCst);

            // Ensure the recovery destination is inside the valid user mapping
            if recovery_rip >= crate::memory::address_space::USER_CODE_VIRT_ADDR
                && recovery_rip < (crate::memory::address_space::USER_CODE_VIRT_ADDR + 4096)
            {
                unsafe {
                    stack_frame.as_mut().update(|f| {
                        f.instruction_pointer = VirtAddr::new(recovery_rip);
                    });
                }
                return;
            }
        }
    }

    error!(
        "[FATAL CPU EXCEPTION: PAGE FAULT] Accessed Address: {:?}\nError Code: {:?}\n{:#?}",
        fault_addr,
        error_code,
        stack_frame
    );
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(
    mut stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let is_user_mode = (stack_frame.code_segment & 3) == 3;

    if is_user_mode && EXPECTED_USER_GPF.load(Ordering::SeqCst) {
        USER_GPF_COUNT.fetch_add(1, Ordering::SeqCst);
        let recovery_rip = GPF_RECOVERY_RIP.load(Ordering::SeqCst);
        if recovery_rip != 0 {
            unsafe {
                stack_frame.as_mut().update(|f| {
                    f.instruction_pointer = VirtAddr::new(recovery_rip);
                });
            }
            return;
        }
    }

    error!(
        "[FATAL CPU EXCEPTION: GENERAL PROTECTION FAULT] Error Code: {}\n{:#?}",
        error_code, stack_frame
    );
    loop {
        x86_64::instructions::hlt();
    }
}
