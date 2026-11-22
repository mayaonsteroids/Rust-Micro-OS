//! Low-level CPU context switching, task stacks, and trampoline routines.
//!
//! Provides the architectural glue between the cooperative Round Robin scheduler
//! and the x86_64 CPU hardware registers.

use core::arch::naked_asm;
use crate::task::task::TaskFn;
use crate::task::manager::MAX_TASKS;

/// Size of each task's private execution stack in bytes (4 KiB = 1 standard x86 page).
pub const STACK_SIZE: usize = 4096;

/// A 16-byte aligned task stack buffer conforming to the System V AMD64 ABI.
#[repr(C, align(16))]
pub struct TaskStack {
    pub data: [u8; STACK_SIZE],
}

impl TaskStack {
    /// Creates a zero-initialized stack buffer.
    pub const fn new() -> Self {
        Self {
            data: [0; STACK_SIZE],
        }
    }

    /// Returns the initial top-of-stack address (high memory, stack grows downward).
    pub fn top(&self) -> u64 {
        let ptr = self.data.as_ptr() as u64;
        ptr + STACK_SIZE as u64
    }
}

/// Statically allocated stacks for all kernel tasks (no dynamic heap required).
pub static mut TASK_STACKS: [TaskStack; MAX_TASKS] = [
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
    TaskStack::new(), TaskStack::new(), TaskStack::new(), TaskStack::new(),
];

/// Saved stack pointers (RSP) for each task slot, enabling deadlock-free context switches.
pub static mut SAVED_RSP: [u64; MAX_TASKS] = [0; MAX_TASKS];

/// Saved stack pointer for the kernel main boot thread.
pub static mut KERNEL_MAIN_RSP: u64 = 0;

/// Reads the current CPU stack pointer (RSP) register directly from hardware.
#[inline(always)]
pub fn current_rsp() -> u64 {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack));
    }
    rsp
}

/// Minimum safety headroom in bytes reserved at the bottom of the stack.
///
/// Saving CPU context requires saving the return address (8 bytes from `call switch_context`)
/// plus 7 callee-saved 64-bit registers (56 bytes: `rflags`, `rbp`, `rbx`, `r12`..`r15`).
/// This demands at least 64 bytes of available stack space above `base`.
/// An RSP below `base + STACK_MIN_HEADROOM` cannot safely perform a context switch without
/// overflowing below `base` and corrupting neighboring memory.
pub const STACK_MIN_HEADROOM: u64 = 64;

/// Validates that a saved stack pointer falls strictly within the task slot's private stack buffer
/// with sufficient safety headroom above the lower boundary.
///
/// Ensures:
/// 1. `slot_index` is within the valid range `0..MAX_TASKS`.
/// 2. `rsp` is non-null (`rsp != 0`).
/// 3. `rsp` satisfies `base + STACK_MIN_HEADROOM <= rsp < top`.
///    - Rejects `rsp == base` (0 bytes headroom remaining; any function call corrupts memory below `base`).
///    - Rejects `rsp < base + STACK_MIN_HEADROOM` (insufficient room for `switch_context`'s 64-byte frame).
///    - Rejects `rsp == top` or `rsp > top` (exceeds or equals the 4096-byte upper bound).
pub fn is_valid_task_rsp(slot_index: usize, rsp: u64) -> bool {
    if slot_index >= MAX_TASKS || rsp == 0 {
        return false;
    }
    let base = unsafe { TASK_STACKS[slot_index].data.as_ptr() as u64 };
    let top = unsafe { TASK_STACKS[slot_index].top() };
    rsp >= base + STACK_MIN_HEADROOM && rsp < top
}

/// Initializes a task's private stack frame so that a context switch cleanly "returns"
/// into `task_trampoline`, which then executes the task's entry point function.
///
/// # Stack Layout upon Initial Switch
/// The buffer is 16-byte aligned (`stack_top % 16 == 0`).
/// We write 9 64-bit words (72 bytes total) at the top of the stack:
/// - `stack_top - 8`:  0x0 (dummy return address, provides System V ABI 16-byte alignment)
/// - `stack_top - 16`: `task_trampoline` entry address (popped into RIP by `ret`)
/// - `stack_top - 24`: 0x0002 (RFLAGS: Reserved bit 1=1, IF=0, interrupts strictly disabled)
/// - `stack_top - 32`: 0x0 (RBP)
/// - `stack_top - 40`: 0x0 (RBX)
/// - `stack_top - 48`: 0x0 (R12)
/// - `stack_top - 56`: 0x0 (R13)
/// - `stack_top - 64`: 0x0 (R14)
/// - `stack_top - 72`: 0x0 (R15)
///
/// When `switch_context` runs, it pops:
/// 1. `r15` (0)
/// 2. `r14` (0)
/// 3. `r13` (0)
/// 4. `r12` (0)
/// 5. `rbx` (0)
/// 6. `rbp` (0)
/// 7. `popfq` loads `rflags` (0x0002: interrupts remain disabled)
/// 8. `ret` pops `task_trampoline` into `RIP`, leaving `%rsp` at `stack_top - 8`.
///
/// At the entry of `task_trampoline`, `(%rsp + 8) % 16 == 0` and `[RSP] == 0`,
/// strictly fulfilling the System V AMD64 ABI specification as if `task_trampoline`
/// were invoked via a standard `call` instruction.
pub unsafe fn setup_task_stack(slot_index: usize, _entry: Option<TaskFn>) -> u64 {
    let stack_top = TASK_STACKS[slot_index].top() as *mut u64;

    let sp = stack_top.sub(9);

    *stack_top.sub(1) = 0;                                              // Dummy return address
    *stack_top.sub(2) = task_trampoline as *const () as usize as u64;   // Return address (RIP)
    *stack_top.sub(3) = 0x0002;                                         // RFLAGS (IF=0, interrupts disabled)
    *stack_top.sub(4) = 0;                                              // RBP
    *stack_top.sub(5) = 0;                                              // RBX
    *stack_top.sub(6) = 0;                                              // R12
    *stack_top.sub(7) = 0;                                              // R13
    *stack_top.sub(8) = 0;                                              // R14
    *stack_top.sub(9) = 0;                                              // R15

    let initial_rsp = sp as u64;
    SAVED_RSP[slot_index] = initial_rsp;
    initial_rsp
}

/// Trampoline function that wraps the execution of bare-metal cooperative tasks.
///
/// When a new task is switched to for the first time:
/// 1. `switch_context` returns directly to this function.
/// 2. This function invokes the task's user-defined entry point function.
/// 3. When the task function finishes its work and returns, this trampoline automatically
///    invokes `crate::scheduler::sys_exit()` to terminate the task and schedule the next task.
///
/// This eliminates stack corruption or general protection faults from task functions returning
/// into invalid memory.
pub extern "C" fn task_trampoline() {
    if let Some(entry) = crate::scheduler::current_task_entry() {
        entry();
    }
    // Cleanly terminate the task when it completes its function
    crate::scheduler::sys_exit();

    // Defensive halt in the impossible event sys_exit returns
    loop {
        x86_64::instructions::hlt();
    }
}

/// Core architectural context-switching routine written in naked x86_64 assembly.
///
/// # Arguments
/// - `old_rsp` (`%rdi`): Pointer to the location where the outgoing task's RSP is saved.
/// - `new_rsp` (`%rsi`): The incoming task's stack pointer to restore into `%rsp`.
///
/// # How It Works
/// 1. Pushes all callee-saved registers (`rflags`, `rbp`, `rbx`, `r12`, `r13`, `r14`, `r15`)
///    onto the current stack.
/// 2. Stores the updated stack pointer into `*old_rsp`.
/// 3. Replaces `%rsp` with `new_rsp` (switching the active CPU stack).
/// 4. Pops the new task's callee-saved registers from the new stack.
/// 5. Executes `ret`, popping the saved return address into `%rip`.
///
/// If the new task previously called `switch_context`, it resumes immediately after that call.
/// If the new task is starting for the first time, it jumps to `task_trampoline`.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(old_rsp: *mut u64, new_rsp: u64) {
    naked_asm!(
        // 1. Save outgoing task's callee-saved context
        "pushfq",
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // 2. Save outgoing task's stack pointer to *old_rsp (RDI)
        "mov [rdi], rsp",

        // 3. Switch CPU stack pointer to new_rsp (RSI)
        "mov rsp, rsi",

        // 4. Restore incoming task's callee-saved context
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "popfq",

        // 5. Return to incoming task's saved RIP
        "ret"
    );
}

/// Reads the current Code Segment (CS) register directly from the CPU.
#[inline(always)]
pub fn current_cs() -> u64 {
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack));
    }
    cs as u64
}

/// Reads the current Stack Segment (SS) register directly from the CPU.
#[inline(always)]
pub fn current_ss() -> u64 {
    let ss: u16;
    unsafe {
        core::arch::asm!("mov {:x}, ss", out(reg) ss, options(nomem, nostack));
    }
    ss as u64
}

/// Initializes a task's private stack frame for timer-driven preemptive scheduling.
///
/// In preemptive mode, tasks run with interrupts enabled (`IF=1` in RFLAGS) and are resumed
/// using `iretq` (hardware interrupt return).
///
/// We construct a full 160-byte frame (20 64-bit words) at the top of the stack:
/// - Words 1..5:  Hardware interrupt frame (SS, RSP, RFLAGS with IF=1, CS, RIP = task_trampoline)
/// - Words 6..20: Software frame (15 GPRs: r15..rax, initialized to 0)
pub unsafe fn setup_preemptive_task_stack(slot_index: usize, _entry: Option<TaskFn>) -> u64 {
    let stack_top = TASK_STACKS[slot_index].top() as *mut u64;

    // 20 words = 160 bytes
    let sp = stack_top.sub(20);

    // Hardware frame (popped by iretq)
    *stack_top.sub(1) = current_ss();
    *stack_top.sub(2) = (stack_top.sub(1)) as usize as u64;                // RSP at trampoline entry
    *stack_top.sub(3) = 0x0202;                                             // RFLAGS (IF=1, Reserved=1)
    *stack_top.sub(4) = current_cs();
    *stack_top.sub(5) = task_trampoline as *const () as usize as u64;       // RIP

    // Software frame (15 GPRs popped by pop rax..r15)
    for i in 6..=20 {
        *stack_top.sub(i) = 0;
    }

    let initial_rsp = sp as u64;
    SAVED_RSP[slot_index] = initial_rsp;
    initial_rsp
}

/// Naked assembly entry point for the PIT timer hardware interrupt (Vector 32, IRQ 0).
///
/// 1. CPU hardware pushes: SS, RSP, RFLAGS, CS, RIP (40 bytes).
/// 2. CPU clears IF=0.
/// 3. Assembly pushes all 15 general-purpose registers (120 bytes).
///    Total frame: 160 bytes (10 * 16B: preserves 16-byte stack alignment).
/// 4. Calls Rust `timer_interrupt_handler(rsp)` with `%rdi = %rsp`.
/// 5. Handler updates ticks, sends PIC EOI, performs scheduling if due, returns next RSP in `%rax`.
/// 6. Assembly sets `%rsp = %rax`.
/// 7. Restores 15 GPRs.
/// 8. Executes `iretq` to resume interrupted or newly dispatched task.
#[unsafe(naked)]
pub unsafe extern "C" fn timer_interrupt_entry() {
    naked_asm!(
        // Save 15 general-purpose registers
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rbp",
        "push rbx",
        "push rdx",
        "push rcx",
        "push rax",

        // Ensure Direction Flag (DF) is clear conforming to System V AMD64 ABI
        "cld",

        // Pass current stack pointer as argument 1 (%rdi) to Rust handler
        "mov rdi, rsp",
        "call timer_interrupt_handler",

        // Switch CPU stack pointer to return value (%rax)
        "mov rsp, rax",

        // Restore 15 general-purpose registers
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rbp",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        // Return from interrupt (hardware pops RIP, CS, RFLAGS, RSP, SS)
        "iretq"
    );
}

/// Switches execution from the kernel boot thread to the first preemptive task via `iretq`.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_main_to_preemptive(main_rsp_ptr: *mut u64, first_rsp: u64) {
    naked_asm!(
        // Save kernel main thread callee-saved registers
        "pushfq",
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp",

        // Switch to first task's preemptive stack
        "mov rsp, rsi",

        // Restore task's initial registers
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rbp",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",

        // Enter task via iretq with interrupts enabled
        "iretq"
    );
}

/// Returns execution from preemptive task mode back to the kernel boot thread.
#[unsafe(naked)]
pub unsafe extern "C" fn return_to_main_from_preemptive(main_rsp: u64) -> ! {
    naked_asm!(
        "mov rsp, rdi",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "popfq",
        "ret"
    );
}

/// Switches CPU execution directly to a preemptive task whose stack holds an interrupt frame.
#[unsafe(naked)]
pub unsafe extern "C" fn switch_to_preemptive_task(next_rsp: u64) -> ! {
    naked_asm!(
        "mov rsp, rdi",
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rbp",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        "iretq"
    );
}
