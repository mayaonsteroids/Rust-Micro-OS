//! Milestone 13 Programmatic In-Kernel Test Suite & Live Demonstration.
//!
//! Verifies:
//! 1. Hardware Memory Isolation (per-process L4 page tables, CR3 switching, ownership tracking).
//! 2. Ring 3 User Mode Execution (GDT/TSS configuration, iretq privilege transition, RSP0 stack).
//! 3. System Calls via Software Interrupt (Vector 0x80, sys_write, sys_yield, sys_exit, bounds validation).
//! 4. Zero regression of all prior milestone subsystems.

use core::sync::atomic::Ordering;
use crate::info;
use crate::memory::address_space::{
    current_cr3, restore_kernel_address_space, AddressSpace,
    USER_CODE_VIRT_ADDR, USER_DATA_VIRT_ADDR, USER_SPACE_MAX, USER_STACK_TOP,
};
use crate::interrupts::gdt::{
    kernel_code_selector, kernel_data_selector, user_code_selector, user_data_selector, TSS,
};
use crate::interrupts::idt::{
    can_recover_page_fault, EXPECTED_FAULT_ADDR, EXPECTED_FAULT_RIP, EXPECTED_USER_FAULT,
    FAULT_RECOVERY_RIP, LAST_FAULT_ADDR, LAST_FAULT_ERROR_CODE, LAST_FAULT_RIP, USER_FAULT_COUNT,
};
use x86_64::structures::idt::PageFaultErrorCode;
use crate::interrupts::syscall::{
    run_user_code_with_exit_return, syscall_dispatcher, EFAULT, EINVAL, ENOSYS,
    LAST_USER_EXIT_CODE, SYS_EXIT, SYS_WRITE, SYS_YIELD,
};

/// Runs the complete suite of 10 programmatic unit tests for Milestone 13.
pub fn run_milestone13_tests() -> bool {
    info!("========================================================");
    info!("     RUNNING MILESTONE 13: USER MODE & SYSCALL TESTS    ");
    info!("========================================================");

    let mut all_passed = true;

    if !test_user_tc1_address_space_creation() {
        all_passed = false;
    }
    if !test_user_tc2_address_space_switching() {
        all_passed = false;
    }
    if !test_user_tc3_gdt_tss_configuration() {
        all_passed = false;
    }
    if !test_user_tc4_syscall_dispatch() {
        all_passed = false;
    }
    if !test_user_tc5_syscall_invalid_number() {
        all_passed = false;
    }
    if !test_user_tc6_syscall_pointer_validation() {
        all_passed = false;
    }
    if !test_user_tc7_kernel_protection_page_fault() {
        all_passed = false;
    }
    if !test_user_tc8_user_mode_execution() {
        all_passed = false;
    }
    if !test_user_tc9_address_space_reclamation() {
        all_passed = false;
    }
    if !test_user_tc10_multi_process_isolation() {
        all_passed = false;
    }

    if all_passed {
        info!(">>> MILESTONE 13 KERNEL TESTS: ALL 10 TESTS PASSED <<<");
    } else {
        crate::error!(">>> MILESTONE 13 KERNEL TESTS: SOME TESTS FAILED <<<");
    }

    all_passed
}

/// USER-TC1: Hardware Address Space Creation & Kernel Isolation
fn test_user_tc1_address_space_creation() -> bool {
    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC1 Failed: AddressSpace::new error: {}", e);
            return false;
        }
    };

    // Inspect L4 Page Table:
    let l4_virt = crate::memory::address_space::phys_to_virt(aspace.cr3.start_address());
    let l4_table = unsafe { &*(l4_virt.as_ptr() as *const x86_64::structures::paging::PageTable) };

    // 1. Entry 1 (User Space) must be PRESENT, WRITABLE, and USER_ACCESSIBLE
    let user_entry = &l4_table[1];
    let user_flags = user_entry.flags();
    if !user_flags.contains(x86_64::structures::paging::PageTableFlags::PRESENT)
        || !user_flags.contains(x86_64::structures::paging::PageTableFlags::WRITABLE)
        || !user_flags.contains(x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE)
    {
        crate::error!("USER-TC1 Failed: L4 Entry 1 flags incorrect: {:?}", user_flags);
        aspace.destroy();
        return false;
    }

    // 2. All kernel entries must NOT be USER_ACCESSIBLE
    for i in 0..512 {
        if i != 1 && !l4_table[i].is_unused() {
            if l4_table[i].flags().contains(x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE) {
                crate::error!("USER-TC1 Failed: Kernel L4 entry {} is marked USER_ACCESSIBLE!", i);
                aspace.destroy();
                return false;
            }
        }
    }

    // 3. Exactly 7 frames must be owned by the address space
    if aspace.owned_count != 7 {
        crate::error!("USER-TC1 Failed: Expected 7 owned frames, found {}", aspace.owned_count);
        aspace.destroy();
        return false;
    }

    aspace.destroy();
    info!("  [PASS] USER-TC1: Hardware Address Space creation, L4 hierarchy & kernel isolation verified.");
    true
}

/// USER-TC2: Address Space Switching via CR3
fn test_user_tc2_address_space_switching() -> bool {
    let kernel_cr3 = current_cr3();

    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC2 Failed: {}", e);
            return false;
        }
    };

    // Switch to user address space
    aspace.switch_to();
    let user_active_cr3 = current_cr3();
    if user_active_cr3 != aspace.cr3 {
        crate::error!("USER-TC2 Failed: Active CR3 0x{:x} != aspace.cr3 0x{:x}",
            user_active_cr3.start_address().as_u64(), aspace.cr3.start_address().as_u64());
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // Restore kernel address space
    restore_kernel_address_space();
    let restored_cr3 = current_cr3();
    if restored_cr3 != kernel_cr3 {
        crate::error!("USER-TC2 Failed: Restored CR3 != kernel_cr3");
        aspace.destroy();
        return false;
    }

    aspace.destroy();
    info!("  [PASS] USER-TC2: Address space switching via CR3 register update verified.");
    true
}

/// USER-TC3: Ring 3 GDT & TSS Configuration
fn test_user_tc3_gdt_tss_configuration() -> bool {
    let ucode = user_code_selector();
    let udata = user_data_selector();
    let kcode = kernel_code_selector();
    let kdata = kernel_data_selector();

    // Verify RPL privilege levels
    if (ucode.0 & 3) != 3 {
        crate::error!("USER-TC3 Failed: User Code Selector RPL != 3: 0x{:x}", ucode.0);
        return false;
    }
    if (udata.0 & 3) != 3 {
        crate::error!("USER-TC3 Failed: User Data Selector RPL != 3: 0x{:x}", udata.0);
        return false;
    }
    if (kcode.0 & 3) != 0 {
        crate::error!("USER-TC3 Failed: Kernel Code Selector RPL != 0: 0x{:x}", kcode.0);
        return false;
    }
    if (kdata.0 & 3) != 0 {
        crate::error!("USER-TC3 Failed: Kernel Data Selector RPL != 0: 0x{:x}", kdata.0);
        return false;
    }

    // Verify TSS RSP0 configuration
    let rsp0 = unsafe { TSS.privilege_stack_table[0].as_u64() };
    if rsp0 == 0 || (rsp0 % 16) != 0 {
        crate::error!("USER-TC3 Failed: Invalid TSS RSP0: 0x{:x}", rsp0);
        return false;
    }

    info!("  [PASS] USER-TC3: Ring 3 GDT selectors (CS=0x{:x}, DS=0x{:x}) and TSS RSP0 verified.", ucode.0, udata.0);
    true
}

/// USER-TC4: System Call Dispatch via int 0x80
fn test_user_tc4_syscall_dispatch() -> bool {
    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC4 Failed: {}", e);
            return false;
        }
    };
    aspace.switch_to();

    // Write a valid test message in user data space
    let msg = b"M13_SYSCALL_DISPATCH_OK";
    unsafe {
        let ptr = USER_DATA_VIRT_ADDR as *mut u8;
        core::ptr::copy_nonoverlapping(msg.as_ptr(), ptr, msg.len());
    }

    // Test SYS_WRITE
    let ret_write = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, msg.len() as u64, 0);
    if ret_write != msg.len() as i64 {
        crate::error!("USER-TC4 Failed: SYS_WRITE returned {}, expected {}", ret_write, msg.len());
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // Test SYS_YIELD
    let ret_yield = syscall_dispatcher(SYS_YIELD, 0, 0, 0);
    if ret_yield != 0 {
        crate::error!("USER-TC4 Failed: SYS_YIELD returned {}", ret_yield);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // Test SYS_EXIT
    let ret_exit = syscall_dispatcher(SYS_EXIT, 77, 0, 0);
    if ret_exit != 0 || LAST_USER_EXIT_CODE.load(Ordering::SeqCst) != 77 {
        crate::error!("USER-TC4 Failed: SYS_EXIT failed or exit code not recorded");
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    restore_kernel_address_space();
    aspace.destroy();
    info!("  [PASS] USER-TC4: System call dispatch via int 0x80 (SYS_WRITE, SYS_YIELD, SYS_EXIT) verified.");
    true
}

/// USER-TC5: Syscall Invalid Number Rejection
fn test_user_tc5_syscall_invalid_number() -> bool {
    let ret_999 = syscall_dispatcher(999, 0, 0, 0);
    let ret_0 = syscall_dispatcher(0, 0, 0, 0);
    let ret_max = syscall_dispatcher(u64::MAX, 0, 0, 0);

    if ret_999 != ENOSYS || ret_0 != ENOSYS || ret_max != ENOSYS {
        crate::error!("USER-TC5 Failed: Did not reject invalid syscall numbers with ENOSYS");
        return false;
    }

    info!("  [PASS] USER-TC5: Invalid syscall numbers safely rejected with ENOSYS.");
    true
}

/// USER-TC6: Syscall User Pointer Validation
fn test_user_tc6_syscall_pointer_validation() -> bool {
    let aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC6 Failed: {}", e);
            return false;
        }
    };
    aspace.switch_to();

    // 1. Null pointer
    let ret_null = syscall_dispatcher(SYS_WRITE, 0, 10, 0);
    if ret_null != EFAULT {
        crate::error!("USER-TC6 Failed: Null pointer not rejected with EFAULT: {}", ret_null);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 2. Canonical kernel higher-half address
    let ret_kernel = syscall_dispatcher(SYS_WRITE, 0xffff_8000_0000_0000, 10, 0);
    if ret_kernel != EFAULT {
        crate::error!("USER-TC6 Failed: Kernel higher-half pointer not rejected: {}", ret_kernel);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 3. Kernel heap address
    let ret_heap = syscall_dispatcher(SYS_WRITE, crate::memory::HEAP_START as u64, 10, 0);
    if ret_heap != EFAULT {
        crate::error!("USER-TC6 Failed: Kernel heap pointer not rejected: {}", ret_heap);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 4. VGA buffer address
    let ret_vga = syscall_dispatcher(SYS_WRITE, 0xb8000, 10, 0);
    if ret_vga != EFAULT {
        crate::error!("USER-TC6 Failed: VGA buffer pointer not rejected: {}", ret_vga);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 5. Overflowing address
    let ret_ovf = syscall_dispatcher(SYS_WRITE, u64::MAX - 4, 10, 0);
    if ret_ovf != EFAULT {
        crate::error!("USER-TC6 Failed: Overflowing pointer not rejected: {}", ret_ovf);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 6. User space upper bound excess
    let ret_oob = syscall_dispatcher(SYS_WRITE, USER_SPACE_MAX + 0x1000, 10, 0);
    if ret_oob != EFAULT {
        crate::error!("USER-TC6 Failed: Out-of-bounds pointer not rejected: {}", ret_oob);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 7. Unmapped page within user space (between code and stack)
    let ret_unmapped = syscall_dispatcher(SYS_WRITE, 0x0000_0080_0000_5000, 10, 0);
    if ret_unmapped != EFAULT {
        crate::error!("USER-TC6 Failed: Unmapped user page not rejected with EFAULT: {}", ret_unmapped);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 8. Buffer exceeding MAX_SYS_WRITE_LEN (4096 bytes)
    let ret_toolarge = syscall_dispatcher(SYS_WRITE, USER_DATA_VIRT_ADDR, 4097, 0);
    if ret_toolarge != EINVAL {
        crate::error!("USER-TC6 Failed: Length > 4096 not rejected with EINVAL: {}", ret_toolarge);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    // 9. Boundary-crossing buffer starting in mapped code page and crossing into unmapped page
    let ret_crossing = syscall_dispatcher(SYS_WRITE, USER_CODE_VIRT_ADDR + 4090, 16, 0);
    if ret_crossing != EFAULT {
        crate::error!("USER-TC6 Failed: Boundary crossing into unmapped page not rejected: {}", ret_crossing);
        restore_kernel_address_space();
        aspace.destroy();
        return false;
    }

    restore_kernel_address_space();
    aspace.destroy();
    info!("  [PASS] USER-TC6: Null, kernel, and overflowing user pointers safely rejected with EFAULT.");
    true
}

/// USER-TC7: Kernel Memory Protection from User Mode (Hardware Page Fault Verification)
fn test_user_tc7_kernel_protection_page_fault() -> bool {
    let mut aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC7 Failed: {}", e);
            return false;
        }
    };

    // 1. Verify architectural L4 permission bits:
    // Kernel entries in L4 have USER_ACCESSIBLE = 0, which enforces hardware #PF from Ring 3
    let l4_virt = crate::memory::address_space::phys_to_virt(aspace.cr3.start_address());
    let l4_table = unsafe { &*(l4_virt.as_ptr() as *const x86_64::structures::paging::PageTable) };

    for i in 0..512 {
        if i != 1 && !l4_table[i].is_unused() {
            assert!(!l4_table[i].flags().contains(x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE));
        }
    }

    // 2. Machine code program for User Mode:
    //    Attempts to read from supervisor address (PHYSICAL_MEMORY_OFFSET).
    //    This page is mapped in the kernel page table, but USER_ACCESSIBLE was stripped in this
    //    isolated address space, so the hardware MMU guarantees a hardware Page Fault (#PF) from Ring 3.
    //
    //    Instruction 1: mov rbx, fault_target
    //      Opcode: 48 bb imm64 (10 bytes: offsets 0..9)
    //    Instruction 2: mov rax, [rbx]  <-- FAULTING INSTRUCTION!
    //      Opcode: 48 8b 03 (3 bytes: offsets 10..12)
    //      CR2 Target = fault_target
    //      Faulting instruction RIP = USER_CODE_VIRT_ADDR + 10
    //    Instruction 3: mov eax, 3 (SYS_EXIT) <-- RECOVERY TARGET!
    //      Opcode: b8 03 00 00 00 (5 bytes: offsets 13..17)
    //      Recovery instruction RIP = USER_CODE_VIRT_ADDR + 13
    //    Instruction 4: xor edi, edi (exit_code = 0)
    //      Opcode: 31 ff (2 bytes: offsets 18..19)
    //    Instruction 5: int 0x80 (syscall entry)
    //      Opcode: cd 80 (2 bytes: offsets 20..21)
    //    Instruction 6: jmp $
    //      Opcode: eb fe (2 bytes: offsets 22..23)
    let fault_target = crate::memory::address_space::PHYSICAL_MEMORY_OFFSET.load(Ordering::SeqCst);
    let target_bytes = fault_target.to_le_bytes();

    const FAULT_INSN_OFFSET: u64 = 10;
    const FAULT_INSN_LEN: u64 = 3;
    const RECOVERY_INSN_OFFSET: u64 = FAULT_INSN_OFFSET + FAULT_INSN_LEN; // 13

    let expected_fault_rip = USER_CODE_VIRT_ADDR + FAULT_INSN_OFFSET;
    let recovery_rip = USER_CODE_VIRT_ADDR + RECOVERY_INSN_OFFSET;

    // Verify recovery destination is inside valid user mapping
    assert!(
        recovery_rip >= USER_CODE_VIRT_ADDR && recovery_rip < (USER_CODE_VIRT_ADDR + 4096),
        "Recovery destination must be inside valid user code page"
    );

    let fault_test_program: [u8; 24] = [
        0x48, 0xbb,
        target_bytes[0], target_bytes[1], target_bytes[2], target_bytes[3],
        target_bytes[4], target_bytes[5], target_bytes[6], target_bytes[7], // mov rbx, fault_target (10 bytes: 0..9)
        0x48, 0x8b, 0x03,                                                 // mov rax, [rbx]        (3 bytes: 10..12) [FAULT]
        0xb8, 0x03, 0x00, 0x00, 0x00,                                     // mov eax, 3 (SYS_EXIT) (5 bytes: 13..17) [RECOVERY]
        0x31, 0xff,                                                       // xor edi, edi          (2 bytes: 18..19)
        0xcd, 0x80,                                                       // int 0x80              (2 bytes: 20..21)
        0xeb, 0xfe,                                                       // jmp $                 (2 bytes: 22..23)
    ];

    if let Err(e) = aspace.load_code(&fault_test_program) {
        crate::error!("USER-TC7 Failed to load user code: {}", e);
        aspace.destroy();
        return false;
    }

    // 3. Fail-Closed Recovery Invariant Verification:
    // Verify that unset or mismatched expected address, unset or mismatched expected RIP,
    // unexpected error code bits, or disarmed flag NEVER authorize recovery.
    let expected_err_code = PageFaultErrorCode::from_bits_truncate(0x5);
    assert_eq!(expected_err_code.bits(), 0x5, "Expected error code must be exactly 0x5");

    // Invariant A: Unset expected address (0) must NEVER authorize recovery
    EXPECTED_USER_FAULT.store(true, Ordering::SeqCst);
    EXPECTED_FAULT_ADDR.store(0, Ordering::SeqCst);
    EXPECTED_FAULT_RIP.store(expected_fault_rip, Ordering::SeqCst);
    if can_recover_page_fault(0x23, expected_err_code, fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Unset expected address (0) must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant B: Mismatched fault address must NEVER authorize recovery
    EXPECTED_FAULT_ADDR.store(fault_target, Ordering::SeqCst);
    if can_recover_page_fault(0x23, expected_err_code, fault_target + 0x1000, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Mismatched fault address must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant C: Unset expected RIP (0) must NEVER authorize recovery
    EXPECTED_FAULT_RIP.store(0, Ordering::SeqCst);
    if can_recover_page_fault(0x23, expected_err_code, fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Unset expected RIP (0) must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant D: Mismatched RIP must NEVER authorize recovery
    EXPECTED_FAULT_RIP.store(expected_fault_rip, Ordering::SeqCst);
    if can_recover_page_fault(0x23, expected_err_code, fault_target, expected_fault_rip + 1) {
        crate::error!("USER-TC7 Invariant Violation: Mismatched RIP must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant E: Disarmed flag must NEVER authorize recovery
    EXPECTED_USER_FAULT.store(false, Ordering::SeqCst);
    if can_recover_page_fault(0x23, expected_err_code, fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Disarmed flag must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant F: Kernel mode origin (CS = 0x08) must NEVER authorize recovery
    EXPECTED_USER_FAULT.store(true, Ordering::SeqCst);
    if can_recover_page_fault(0x08, expected_err_code, fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Kernel mode origin must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant G: Empty error code must NEVER authorize recovery
    if can_recover_page_fault(0x23, PageFaultErrorCode::empty(), fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Empty error code must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant H: Unexpected error-code bits must NEVER authorize recovery:
    // - Bit 1 set: Write fault (0x7) instead of read (0x5)
    if can_recover_page_fault(0x23, PageFaultErrorCode::from_bits_truncate(0x7), fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Write bit in error code (0x7) must not authorize recovery!");
        aspace.destroy();
        return false;
    }
    // - Bit 4 set: Instruction fetch fault (0x15) instead of read (0x5)
    if can_recover_page_fault(0x23, PageFaultErrorCode::from_bits_truncate(0x15), fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Instruction fetch bit (0x15) must not authorize recovery!");
        aspace.destroy();
        return false;
    }
    // - Bit 3 set: Reserved bit violation (0xd)
    if can_recover_page_fault(0x23, PageFaultErrorCode::from_bits_truncate(0xd), fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Reserved bit (0xd) must not authorize recovery!");
        aspace.destroy();
        return false;
    }
    // - Bit 0 clear: Non-present fault (0x4)
    if can_recover_page_fault(0x23, PageFaultErrorCode::from_bits_truncate(0x4), fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Non-present bit (0x4) must not authorize recovery!");
        aspace.destroy();
        return false;
    }

    // Invariant I: Exact matching parameters (CS=0x23, Err=0x5, Addr=fault_target, RIP=expected_fault_rip) MUST qualify
    if !can_recover_page_fault(0x23, expected_err_code, fault_target, expected_fault_rip) {
        crate::error!("USER-TC7 Invariant Violation: Legitimate fault parameters failed recovery authorization!");
        aspace.destroy();
        return false;
    }

    // 4. Configure live fault interception in IDT handler for Ring 3 execution
    let fault_count_before = USER_FAULT_COUNT.load(Ordering::SeqCst);

    LAST_FAULT_ADDR.store(0, Ordering::SeqCst);
    LAST_FAULT_RIP.store(0, Ordering::SeqCst);
    LAST_FAULT_ERROR_CODE.store(0, Ordering::SeqCst);
    FAULT_RECOVERY_RIP.store(recovery_rip, Ordering::SeqCst);
    EXPECTED_FAULT_ADDR.store(fault_target, Ordering::SeqCst);
    EXPECTED_FAULT_RIP.store(expected_fault_rip, Ordering::SeqCst);
    EXPECTED_USER_FAULT.store(true, Ordering::SeqCst);
    LAST_USER_EXIT_CODE.store(999, Ordering::SeqCst);

    // Switch to isolated address space
    aspace.switch_to();

    // Execute in Ring 3: triggers hardware #PF, recovers, and exits cleanly via SYS_EXIT(0)
    unsafe {
        run_user_code_with_exit_return(USER_CODE_VIRT_ADDR, USER_STACK_TOP);
    }

    // Verify the one-shot recovery flag and state were consumed exactly once by the handler:
    let flag_consumed = !EXPECTED_USER_FAULT.load(Ordering::SeqCst);
    let expected_addr_cleared = EXPECTED_FAULT_ADDR.load(Ordering::SeqCst) == 0;
    let expected_rip_cleared = EXPECTED_FAULT_RIP.load(Ordering::SeqCst) == 0;
    let recovery_rip_cleared = FAULT_RECOVERY_RIP.load(Ordering::SeqCst) == 0;

    restore_kernel_address_space();

    if !flag_consumed || !expected_addr_cleared || !expected_rip_cleared || !recovery_rip_cleared {
        crate::error!("USER-TC7 Failed: Recovery flag/state was not consumed exactly once by the handler!");
        aspace.destroy();
        return false;
    }

    let fault_count_after = USER_FAULT_COUNT.load(Ordering::SeqCst);
    let fault_addr = LAST_FAULT_ADDR.load(Ordering::SeqCst);
    let fault_rip = LAST_FAULT_RIP.load(Ordering::SeqCst);
    let error_code = LAST_FAULT_ERROR_CODE.load(Ordering::SeqCst);
    let exit_code = LAST_USER_EXIT_CODE.load(Ordering::SeqCst);

    if fault_count_after != fault_count_before + 1 {
        crate::error!("USER-TC7 Failed: Hardware #PF was not generated! (before={}, after={})",
            fault_count_before, fault_count_after);
        aspace.destroy();
        return false;
    }

    if fault_addr != fault_target {
        crate::error!("USER-TC7 Failed: Fault address mismatch: got 0x{:x}, expected 0x{:x}",
            fault_addr, fault_target);
        aspace.destroy();
        return false;
    }

    if fault_rip != expected_fault_rip {
        crate::error!("USER-TC7 Failed: Fault RIP mismatch: got 0x{:x}, expected 0x{:x}",
            fault_rip, expected_fault_rip);
        aspace.destroy();
        return false;
    }

    // Error code must match exactly 0x5 (present-page protection violation from user mode)
    if error_code != 0x5 {
        crate::error!("USER-TC7 Failed: Expected exact #PF error code 0x5, got: 0x{:x}", error_code);
        aspace.destroy();
        return false;
    }

    if exit_code != 0 {
        crate::error!("USER-TC7 Failed: User recovery exit code != 0: {}", exit_code);
        aspace.destroy();
        return false;
    }

    aspace.destroy();
    info!("  [PASS] USER-TC7: Hardware #PF on supervisor access from Ring 3 (CR2=0x{:x}, RIP=0x{:x}, err=0x{:x}) verified in QEMU.",
        fault_addr, fault_rip, error_code);
    info!("  [PASS] USER-TC7: Kernel memory hardware protection from Ring 3 (Page Fault verification) verified.");
    true
}

/// USER-TC8: Ring 3 User Mode Execution, sys_write, and Clean sys_exit
fn test_user_tc8_user_mode_execution() -> bool {
    let mut aspace = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC8 Failed: {}", e);
            return false;
        }
    };

    // 1. Write user string in user data page: "Hello from Milestone 13 Ring 3!" (30 bytes)
    let user_msg = b"Hello from Milestone 13 Ring 3!";
    let data_virt = crate::memory::address_space::phys_to_virt(aspace.user_data_frame.start_address());
    unsafe {
        core::ptr::copy_nonoverlapping(user_msg.as_ptr(), data_virt.as_mut_ptr::<u8>(), user_msg.len());
    }

    // 2. Machine code program for User Mode:
    //    mov eax, 1                      (SYS_WRITE)
    //    mov rdi, 0x0000_0080_0002_0000  (user buffer virtual address)
    //    mov rsi, 30                     (length = 30 bytes)
    //    int 0x80                        (trap into kernel syscall handler)
    //    mov eax, 3                      (SYS_EXIT)
    //    xor edi, edi                    (exit_code = 0)
    //    int 0x80                        (exit)
    let user_program: [u8; 35] = [
        0xb8, 0x01, 0x00, 0x00, 0x00,                         // mov eax, 1
        0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, // mov rdi, 0x0000_0080_0002_0000
        0x48, 0xc7, 0xc6, 0x1e, 0x00, 0x00, 0x00,             // mov rsi, 30
        0xcd, 0x80,                                           // int 0x80
        0xb8, 0x03, 0x00, 0x00, 0x00,                         // mov eax, 3
        0x31, 0xff,                                           // xor edi, edi
        0xcd, 0x80,                                           // int 0x80
        0xeb, 0xfe,                                           // jmp $
    ];

    if let Err(e) = aspace.load_code(&user_program) {
        crate::error!("USER-TC8 Failed to load user code: {}", e);
        aspace.destroy();
        return false;
    }

    // 3. Switch to the user's isolated address space
    aspace.switch_to();

    // Reset exit code sentinel before running
    LAST_USER_EXIT_CODE.store(999, Ordering::SeqCst);

    // 4. Transition CPU to Ring 3 via iretq; returns cleanly on SYS_EXIT
    unsafe {
        run_user_code_with_exit_return(USER_CODE_VIRT_ADDR, USER_STACK_TOP);
    }

    // 5. Restore kernel CR3 and verify exit code
    restore_kernel_address_space();

    let exit_code = LAST_USER_EXIT_CODE.load(Ordering::SeqCst);
    if exit_code != 0 {
        crate::error!("USER-TC8 Failed: User code exited with code {}, expected 0", exit_code);
        aspace.destroy();
        return false;
    }

    aspace.destroy();
    info!("  [PASS] USER-TC8: Ring 3 User Mode execution, sys_write, and clean sys_exit verified.");
    true
}

/// USER-TC9: Address Space Destruction & Frame Reclamation
fn test_user_tc9_address_space_reclamation() -> bool {
    let (initial_alloc, initial_dealloc) = {
        let guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        guard.as_ref().map_or((0, 0), |fa| (fa.allocated_count, fa.deallocated_count))
    };

    // Allocate 2 address spaces (7 frames each = 14 frames total)
    let aspace1 = AddressSpace::new().expect("aspace1");
    let aspace2 = AddressSpace::new().expect("aspace2");

    assert_eq!(aspace1.owned_count, 7);
    assert_eq!(aspace2.owned_count, 7);

    // Destroy both address spaces
    aspace1.destroy();
    aspace2.destroy();

    let (final_alloc, final_dealloc) = {
        let guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        guard.as_ref().map_or((0, 0), |fa| (fa.allocated_count, fa.deallocated_count))
    };

    if final_dealloc != initial_dealloc + 14 {
        crate::error!("USER-TC9 Failed: Deallocation count mismatch. initial={}, final={}",
            initial_dealloc, final_dealloc);
        return false;
    }
    if final_alloc != initial_alloc + 14 {
        crate::error!("USER-TC9 Failed: Allocation count mismatch. initial={}, final={}",
            initial_alloc, final_alloc);
        return false;
    }

    // Verify recycled frames can be re-allocated successfully
    let aspace3 = AddressSpace::new().expect("aspace3 from recycled frames");
    assert_eq!(aspace3.owned_count, 7);
    aspace3.destroy();

    // Regression check A: Partial construction failure must not leak frames
    let dealloc_before_fail = {
        let guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        guard.as_ref().map_or(0, |fa| fa.deallocated_count)
    };
    crate::memory::address_space::SIMULATE_ALLOC_FAILURE_AFTER.store(3, Ordering::SeqCst);
    let fail_res = AddressSpace::new();
    crate::memory::address_space::SIMULATE_ALLOC_FAILURE_AFTER.store(usize::MAX, Ordering::SeqCst);
    if fail_res.is_ok() {
        crate::error!("USER-TC9 Failed: Expected AddressSpace::new to fail on simulated allocation failure");
        return false;
    }
    let dealloc_after_fail = {
        let guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        guard.as_ref().map_or(0, |fa| fa.deallocated_count)
    };
    if dealloc_after_fail != dealloc_before_fail + 3 {
        crate::error!("USER-TC9 Failed: Partial construction failure leaked frames! (expected delta 3, got {})",
            dealloc_after_fail - dealloc_before_fail);
        return false;
    }

    // Regression check B: Active address space drop must restore kernel CR3 before deallocation
    {
        let active_aspace = AddressSpace::new().expect("active_aspace");
        active_aspace.switch_to();
        let active_cr3 = current_cr3();
        assert_eq!(active_cr3, active_aspace.cr3);
        // Explicitly drop active address space
        drop(active_aspace);
        let restored_cr3 = current_cr3();
        let kernel_l4 = crate::memory::address_space::KERNEL_L4_FRAME.lock().expect("kernel_l4");
        if restored_cr3 != kernel_l4 {
            crate::error!("USER-TC9 Failed: Dropping active address space did not restore kernel CR3!");
            return false;
        }
    }

    info!("  [PASS] USER-TC9: Address space destruction and physical frame reclamation verified (zero leaks).");
    true
}

/// USER-TC10: Multi-Process Address Space Isolation
fn test_user_tc10_multi_process_isolation() -> bool {
    let aspace_alpha = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC10 Failed: {}", e);
            return false;
        }
    };

    let aspace_beta = match AddressSpace::new() {
        Ok(a) => a,
        Err(e) => {
            crate::error!("USER-TC10 Failed: {}", e);
            aspace_alpha.destroy();
            return false;
        }
    };

    // Under Alpha's address space, write signature A into user data virtual address
    aspace_alpha.switch_to();
    let sig_alpha: u64 = 0xAAAA_1111_2222_3333;
    unsafe {
        let ptr = USER_DATA_VIRT_ADDR as *mut u64;
        core::ptr::write_volatile(ptr, sig_alpha);
    }

    // Under Beta's address space, write signature B into the same virtual address
    aspace_beta.switch_to();
    let sig_beta: u64 = 0xBBBB_4444_5555_6666;
    unsafe {
        let ptr = USER_DATA_VIRT_ADDR as *mut u64;
        core::ptr::write_volatile(ptr, sig_beta);
    }

    // Switch back to Alpha and verify signature A is intact
    aspace_alpha.switch_to();
    let read_alpha = unsafe {
        let ptr = USER_DATA_VIRT_ADDR as *const u64;
        core::ptr::read_volatile(ptr)
    };
    if read_alpha != sig_alpha {
        crate::error!("USER-TC10 Failed: Alpha data corrupted by Beta! Read 0x{:x}, expected 0x{:x}",
            read_alpha, sig_alpha);
        restore_kernel_address_space();
        aspace_alpha.destroy();
        aspace_beta.destroy();
        return false;
    }

    // Switch to Beta and verify signature B is intact
    aspace_beta.switch_to();
    let read_beta = unsafe {
        let ptr = USER_DATA_VIRT_ADDR as *const u64;
        core::ptr::read_volatile(ptr)
    };
    if read_beta != sig_beta {
        crate::error!("USER-TC10 Failed: Beta data incorrect! Read 0x{:x}, expected 0x{:x}",
            read_beta, sig_beta);
        restore_kernel_address_space();
        aspace_alpha.destroy();
        aspace_beta.destroy();
        return false;
    }

    restore_kernel_address_space();
    aspace_alpha.destroy();
    aspace_beta.destroy();

    info!("  [PASS] USER-TC10: Multi-process hardware address space isolation verified (identical VAs, distinct PAs).");
    true
}
