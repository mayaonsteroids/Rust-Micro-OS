//! System Call subsystem: Minimal syscall interface, user pointer bounds checking,
//! and software interrupt dispatch (Vector 0x80) conforming to Milestone 13.

use core::arch::naked_asm;
use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use crate::memory::address_space::{is_user_buffer_valid, MAX_SYS_WRITE_LEN};

/// System call numbers:
pub const SYS_WRITE: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_EXIT:  u64 = 3;

/// Architectural System Call error codes (standard negative errno values):
pub const SUCCESS: i64 = 0;
pub const EPERM:   i64 = -1;
pub const EFAULT:  i64 = -14;
pub const EINVAL:  i64 = -22;
pub const ENOSYS:  i64 = -38;

/// Telemetry metrics for system calls:
pub static TOTAL_SYSCALL_COUNT: AtomicU64 = AtomicU64::new(0);
pub static LAST_SYSCALL_NUM:    AtomicU64 = AtomicU64::new(0);
pub static LAST_SYSCALL_RESULT: AtomicI64 = AtomicI64::new(0);
pub static LAST_USER_EXIT_CODE: AtomicU64 = AtomicU64::new(0);

/// Saved kernel stack pointer for returning from isolated user-mode test execution.
pub static USER_TEST_RETURN_RSP: AtomicU64 = AtomicU64::new(0);

/// Central Rust System Call dispatcher called from `syscall_interrupt_entry`.
#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    num: u64,
    arg1: u64,
    arg2: u64,
    _arg3: u64,
) -> i64 {
    TOTAL_SYSCALL_COUNT.fetch_add(1, Ordering::Relaxed);
    LAST_SYSCALL_NUM.store(num, Ordering::Relaxed);

    let result = match num {
        SYS_WRITE => sys_write_handler(arg1, arg2),
        SYS_YIELD => sys_yield_handler(),
        SYS_EXIT  => sys_exit_handler(arg1),
        _ => {
            crate::warn!("[SYSCALL] Unimplemented or invalid syscall number: {}", num);
            ENOSYS
        }
    };

    LAST_SYSCALL_RESULT.store(result, Ordering::Relaxed);
    result
}

/// Handler for `SYS_WRITE` (Syscall 1):
/// Writes `len` bytes from user buffer `user_ptr` to the system console.
///
/// Strictly validates that every page spanned by `[user_ptr .. user_ptr + len]` is
/// present and user-accessible in the active hardware page table hierarchy, preventing
/// any kernel page faults.
pub fn sys_write_handler(user_ptr: u64, len: u64) -> i64 {
    if len == 0 {
        if user_ptr == 0 || !is_user_buffer_valid(user_ptr, 1) {
            return EFAULT;
        }
        return 0;
    }

    if len > MAX_SYS_WRITE_LEN {
        crate::warn!(
            "[SYSCALL sys_write] EINVAL: Length {} exceeds limit {}. Rejected.",
            len, MAX_SYS_WRITE_LEN
        );
        return EINVAL;
    }

    // Reject null pointers, kernel addresses, unmapped pages, or non-canonical addresses
    if !is_user_buffer_valid(user_ptr, len) {
        crate::warn!(
            "[SYSCALL sys_write] EFAULT: Invalid or unmapped user buffer pointer: 0x{:x} (len: {}). Rejected.",
            user_ptr, len
        );
        return EFAULT;
    }

    // Safe to read: every byte is verified mapped and user-accessible in the active page table
    let slice = unsafe { core::slice::from_raw_parts(user_ptr as *const u8, len as usize) };
    if let Ok(s) = core::str::from_utf8(slice) {
        crate::info!("[USER CONSOLE] {}", s);
    } else {
        crate::info!("[USER CONSOLE] (raw {} bytes received)", len);
    }

    len as i64
}

/// Handler for `SYS_YIELD` (Syscall 2):
/// Voluntarily yields remaining CPU time-slice to the scheduler.
pub fn sys_yield_handler() -> i64 {
    crate::info!("[SYSCALL sys_yield] User task voluntarily yielded CPU.");
    crate::scheduler::sys_yield();
    SUCCESS
}

/// Handler for `SYS_EXIT` (Syscall 3):
/// Terminates the calling user task with an exit code.
pub fn sys_exit_handler(exit_code: u64) -> i64 {
    LAST_USER_EXIT_CODE.store(exit_code, Ordering::SeqCst);
    crate::info!("[SYSCALL sys_exit] User task terminated with exit code: {}.", exit_code);
    SUCCESS
}

/// Naked assembly interrupt entry point for software interrupt Vector 0x80 (`int 0x80`).
///
/// Conforms to System V AMD64 ABI:
/// 1. Hardware automatically switches to `TSS.privilege_stack_table[0]` (Kernel Stack).
/// 2. Hardware pushes: User SS, User RSP, User RFLAGS, User CS, User RIP (40 bytes).
/// 3. Software pushes all 15 general-purpose registers (120 bytes).
/// 4. Stack is dynamically aligned to 16 bytes before calling `syscall_dispatcher`.
/// 5. Return value is written to saved RAX on the stack.
/// 6. User data segment (0x1b) is restored into DS and ES before returning.
/// 7. GPRs are restored.
/// 8. `iretq` returns control to User Mode in Ring 3.
#[unsafe(naked)]
pub unsafe extern "C" fn syscall_interrupt_entry() {
    naked_asm!(
        // 1. Save general-purpose registers (System V AMD64 ABI)
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rbp",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",

        // 2. Reload kernel data segment into DS and ES
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",

        // 3. Clear direction flag per ABI
        "cld",

        // 4. Pass arguments to syscall_dispatcher(num: u64, arg1: u64, arg2: u64, arg3: u64)
        // System V ABI: rdi=num, rsi=arg1, rdx=arg2, rcx=arg3
        "mov rdi, [rsp]",       // saved rax (syscall number)
        "mov rsi, [rsp + 8]",   // saved rdi (arg1)
        "mov rdx, [rsp + 16]",  // saved rsi (arg2)
        "mov rcx, [rsp + 24]",  // saved rdx (arg3)

        // 5. Align stack to 16 bytes before calling Rust function
        "mov rbp, rsp",
        "and rsp, -16",
        "call syscall_dispatcher",
        "mov rsp, rbp",

        // Check if the syscall number was SYS_EXIT (3) while in kernel test return mode
        "cmp qword ptr [rsp], 3",
        "jne 2f",
        "mov r11, qword ptr [rip + {return_rsp}]",
        "test r11, r11",
        "jz 2f",
        // Disarm test return pointer
        "mov qword ptr [rip + {return_rsp}], 0",
        // Switch to saved kernel test stack and return cleanly to kernel test runner
        "mov rsp, r11",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "popfq",
        "ret",

        "2:",
        // 6. Overwrite saved RAX with return value from dispatcher
        "mov [rsp], rax",

        // 7. Reload user data segment (0x1b, RPL 3) into DS and ES before returning to Ring 3
        "mov ax, 0x1b",
        "mov ds, ax",
        "mov es, ax",

        // 8. Restore general-purpose registers
        "pop rax",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rbp",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        // 9. Return to User Mode (Ring 3)
        "iretq",
        return_rsp = sym USER_TEST_RETURN_RSP,
    );
}

/// Executes user-mode code and automatically returns to the caller when `SYS_EXIT` is invoked.
///
/// Saves kernel callee-saved registers, configures `USER_TEST_RETURN_RSP`, transitions
/// to Ring 3 via `iretq`, and returns cleanly to this function once the user code calls `SYS_EXIT`.
#[unsafe(naked)]
pub unsafe extern "C" fn run_user_code_with_exit_return(rip: u64, rsp: u64) {
    naked_asm!(
        // 1. Save kernel callee-saved registers
        "pushfq",
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // 2. Save current kernel stack pointer into USER_TEST_RETURN_RSP
        "mov rax, rsp",
        "mov qword ptr [rip + {return_rsp}], rax",

        // 3. Load user data segment (0x1b) into DS, ES, FS, GS
        "mov ax, 0x1b",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

        // 4. Construct 5-word hardware interrupt return frame:
        "push 0x1b",       // SS (User Data)
        "push rsi",        // RSP (User RSP)
        "push 0x0202",     // RFLAGS (IF=1, Reserved=1)
        "push 0x23",       // CS (User Code)
        "push rdi",        // RIP (User RIP)
        "cld",

        // 5. Enter Ring 3!
        "iretq",
        return_rsp = sym USER_TEST_RETURN_RSP,
    );
}

/// Transitions the CPU from Kernel Mode (Ring 0) to User Mode (Ring 3) via `iretq`.
///
/// # Arguments
/// - `rip`: Entry point virtual address in user space (e.g. `USER_CODE_VIRT_ADDR`).
/// - `rsp`: User stack top virtual address (e.g. `USER_STACK_TOP`).
#[unsafe(naked)]
pub unsafe extern "C" fn enter_user_mode(rip: u64, rsp: u64) -> ! {
    naked_asm!(
        // 1. Load user data segment (0x1b = index 3, RPL 3) into DS, ES, FS, GS
        "mov ax, 0x1b",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

        // 2. Construct 5-word hardware interrupt return frame on current stack:
        // SS: User Data Segment selector (0x1b)
        "push 0x1b",
        // RSP: User Stack Pointer (RSI)
        "push rsi",
        // RFLAGS: 0x0202 (IF=1, Reserved bit 1=1)
        "push 0x0202",
        // CS: User Code Segment selector (0x23 = index 4, RPL 3)
        "push 0x23",
        // RIP: User Entry Point (RDI)
        "push rdi",

        // 3. Clear direction flag
        "cld",

        // 4. iretq loads RIP, CS, RFLAGS, RSP, SS and switches CPL to 3!
        "iretq"
    );
}
