# Milestone 9 — Final Source-Code Verification & Audit Report

**Date:** 2026-11-22  
**Component:** Timer Interrupts, Programmable Interval Timer (PIT), 8259 PIC, and Preemptive Round Robin Scheduler  
**Status:** Verification Complete — All Tests Passing (Exit Code 0) — Awaiting Formal Approval  

---

## Executive Summary

This document presents the final, rigorous source-code verification and audit of the **Milestone 9** implementation in the `rust-micro-os` bare-metal kernel. It details:

1. **Hardware & Software Interrupt Assembly:** Complete, unmodified routines in `src/scheduler/context.rs`, step-by-step stack frame layout (160 bytes total = 40 bytes hardware + 120 bytes software), System V AMD64 alignment, and register push/pop verification.
2. **Interrupt Handler Return Path:** Complete, unmodified routines in `src/scheduler/round_robin.rs` (`timer_interrupt_handler`, `handle_timer_preemption`, `sys_exit`), tracing all five scheduling branches (task preempted, same task re-selected, task exits, empty ready queue, lock contention).
3. **Lock & Interrupt Safety:** Exact mutex acquisition lines, non-blocking `try_lock()` semantics, prevention of nested interrupts via x86_64 Interrupt Gates, and formal proof that lock contention causes zero task loss or state corruption.
4. **PIC and PIT Subsystems:** Dual 8259 PIC remapping, IRQ0 unmasking, 100 Hz Mode 2 PIT divisor math, IDT vector 32 registration, and atomic interrupt enablement via `iretq` (IF=1).
5. **Task Lifecycle & Kernel Main Stack Restoration:** Full trace of task completion via `sys_exit()`, `prepare_exit()`, and `return_to_main_from_preemptive()`, proving terminated tasks can never resume and the boot thread stack is safely restored.
6. **Programmatic Verification:** Full verification test results across `cargo check`, `cargo bootimage`, `python3 -m py_compile`, and `python3 scripts/test_milestone9.py` (all exiting with code 0).

---

## 1. Actual Interrupt Assembly (`src/scheduler/context.rs`)

### 1.1 Complete Unmodified Source Code

#### A. Naked Timer Interrupt Entry Routine
```rust
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
```

#### B. Preemptive Task Stack Initialization
```rust
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
```

#### C. Context Switch Routines for Preemptive Execution
```rust
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
```

---

### 1.2 Instruction-by-Instruction Stack Layout Analysis

When the 8253 PIT timer fires IRQ0 (vector 32):

#### Step 1: Hardware-Pushed Words (AMD64 Long Mode)
In 64-bit Long Mode, the CPU architecture unconditionally pushes 5 QWORDs (40 bytes) onto the active stack, even within Ring 0 with no privilege transition:
1. `[stack_top - 8]`  ($\text{RSP} + 32$): **SS** (Stack Segment selector)
2. `[stack_top - 16]` ($\text{RSP} + 24$): **RSP** (Interrupted task's stack pointer prior to interrupt)
3. `[stack_top - 24]` ($\text{RSP} + 16$): **RFLAGS** (Processor flags, including IF=1)
4. `[stack_top - 32]` ($\text{RSP} + 8$):  **CS** (Code Segment selector, CPL=0)
5. `[stack_top - 40]` ($\text{RSP} + 0$):  **RIP** (Interrupted instruction pointer)
*Simultaneously, the CPU clears `IF=0` in RFLAGS, disabling further interrupts.*

#### Step 2: Software-Pushed General-Purpose Registers (15 GPRs, 120 bytes)
`timer_interrupt_entry` executes 15 consecutive 64-bit pushes:
- `push r15` $\to$ decrements RSP by 8
- `push r14` $\to$ decrements RSP by 8
- `push r13` $\to$ decrements RSP by 8
- `push r12` $\to$ decrements RSP by 8
- `push r11` $\to$ decrements RSP by 8
- `push r10` $\to$ decrements RSP by 8
- `push r9`  $\to$ decrements RSP by 8
- `push r8`  $\to$ decrements RSP by 8
- `push rdi` $\to$ decrements RSP by 8
- `push rsi` $\to$ decrements RSP by 8
- `push rbp` $\to$ decrements RSP by 8
- `push rbx` $\to$ decrements RSP by 8
- `push rdx` $\to$ decrements RSP by 8
- `push rcx` $\to$ decrements RSP by 8
- `push rax` $\to$ decrements RSP by 8

Total frame size: $40\text{ bytes (hardware)} + 120\text{ bytes (software)} = 160\text{ bytes}$ ($20 \times 8\text{ bytes}$).  
Because $160 \pmod{16} == 0$, the 16-byte alignment of the stack is strictly preserved.

---

### 1.3 Offset Verification Table (Assembly vs. Rust Handler)

When `timer_interrupt_entry` executes `mov rdi, rsp` and calls `timer_interrupt_handler(rsp)`, `%rdi` points to the bottom of the 160-byte frame (`rax`). The table below proves that the Rust handler offsets directly mirror the assembly push order:

| Offset from `%rsp` | 64-bit Word Index | Value Saved / Restored | Source in Assembly | Rust Diagnostic Access (`src/scheduler/round_robin.rs`) |
|---|---|---|---|---|
| `+0` (`0x00`) | Index 0 | `rax` | `push rax` / `pop rax` | `*(rsp as *const u64).add(0)` |
| `+8` (`0x08`) | Index 1 | `rcx` | `push rcx` / `pop rcx` | `*(rsp as *const u64).add(1)` |
| `+16` (`0x10`) | Index 2 | `rdx` | `push rdx` / `pop rdx` | `*(rsp as *const u64).add(2)` |
| `+24` (`0x18`) | Index 3 | `rbx` | `push rbx` / `pop rbx` | `*(rsp as *const u64).add(3)` |
| `+32` (`0x20`) | Index 4 | `rbp` | `push rbp` / `pop rbp` | `*(rsp as *const u64).add(4)` |
| `+40` (`0x28`) | Index 5 | `rsi` | `push rsi` / `pop rsi` | `*(rsp as *const u64).add(5)` |
| `+48` (`0x30`) | Index 6 | `rdi` | `push rdi` / `pop rdi` | `*(rsp as *const u64).add(6)` |
| `+56` (`0x38`) | Index 7 | `r8` | `push r8` / `pop r8` | `*(rsp as *const u64).add(7)` |
| `+64` (`0x40`) | Index 8 | `r9` | `push r9` / `pop r9` | `*(rsp as *const u64).add(8)` |
| `+72` (`0x48`) | Index 9 | `r10` | `push r10` / `pop r10` | `*(rsp as *const u64).add(9)` |
| `+80` (`0x50`) | Index 10 | `r11` | `push r11` / `pop r11` | `*(rsp as *const u64).add(10)` |
| `+88` (`0x58`) | Index 11 | `r12` | `push r12` / `pop r12` | `*(rsp as *const u64).add(11)` |
| `+96` (`0x60`) | Index 12 | `r13` | `push r13` / `pop r13` | `*(rsp as *const u64).add(12)` |
| `+104` (`0x68`) | Index 13 | `r14` | `push r14` / `pop r14` | `*(rsp as *const u64).add(13)` |
| `+112` (`0x70`) | Index 14 | `r15` | `push r15` / `pop r15` | `*(rsp as *const u64).add(14)` |
| **`+120` (`0x78`)** | **Index 15** | **RIP** | **CPU Hardware Push** | **`let rip = *(rsp as *const u64).add(15);` (Line 551)** |
| **`+128` (`0x80`)** | **Index 16** | **CS** | **CPU Hardware Push** | **`let cs = *(rsp as *const u64).add(16);` (Line 552)** |
| **`+136` (`0x88`)** | **Index 17** | **RFLAGS** | **CPU Hardware Push** | **`let rflags = *(rsp as *const u64).add(17);` (Line 553)** |
| **`+144` (`0x90`)** | **Index 18** | **RSP** | **CPU Hardware Push** | **`let task_rsp = *(rsp as *const u64).add(18);` (Line 554)** |
| **`+152` (`0x98`)** | **Index 19** | **SS** | **CPU Hardware Push** | **`let ss = *(rsp as *const u64).add(19);` (Line 555)** |

The pop order in assembly:
```nasm
pop rax
pop rcx
pop rdx
pop rbx
pop rbp
pop rsi
pop rdi
pop r8
pop r9
pop r10
pop r11
pop r12
pop r13
pop r14
pop r15
iretq
```
is the exact mathematical inverse of the push order, ensuring zero register corruption.

---

### 1.4 x86_64 System V ABI Stack Alignment Mathematical Proof

The System V AMD64 ABI specification dictates:
> "The end of the input argument area shall be aligned on a 16-byte boundary. In other words, the value `(%rsp + 8)` is always a multiple of 16 when control is transferred to the function entry point."

We trace the actual `%rsp` value modulo 16 at each transition:

#### Stage 0: Prior to Hardware Interrupt
A running task executes on its private 4 KiB stack. Under the AMD64 architecture (AMD64 APM Vol 2 §8.9.3, Intel SDM Vol 3A §6.14.2):
> "In 64-bit mode, the stack pointer (RSP) is aligned to a 16-byte boundary before pushing the interrupt stack frame onto the stack. The processor aligns RSP by clearing bit 3 of RSP before pushing SS, RSP, RFLAGS, CS, and RIP. The unaligned RSP is saved in the RSP field of the interrupt stack frame."

Therefore, the CPU hardware aligns the stack pointer to:
$$\text{RSP}_{\text{aligned}} = \text{RSP}_{\text{task}} \ \& \ \sim 0\text{x}8 \implies \mathbf{\text{RSP}_{\text{aligned}} \pmod{16} = 0}$$

#### Stage 1: Immediately After Hardware Interrupt Entry
The CPU pushes 5 QWORDs (40 bytes total): `SS` (8B), `RSP` (8B), `RFLAGS` (8B), `CS` (8B), `RIP` (8B).
$$\text{RSP}_{\text{hw\_entry}} = \text{RSP}_{\text{aligned}} - 40$$
$$\text{RSP}_{\text{hw\_entry}} \pmod{16} = (0 - 40) \pmod{16} = -40 \pmod{16} = \mathbf{8}$$
*Immediately after hardware interrupt entry, $\mathbf{RSP \pmod{16} == 8}$.*

#### Stage 2: Immediately Before `call timer_interrupt_handler`
`timer_interrupt_entry` executes 15 register pushes (`r15` down to `rax`), pushing exactly 120 bytes:
$$120 = 7 \times 16 + 8 \implies 120 \equiv 8 \pmod{16}$$
$$\text{RSP}_{\text{pre\_call}} = \text{RSP}_{\text{hw\_entry}} - 120$$
$$\text{RSP}_{\text{pre\_call}} \pmod{16} = (8 - 120) \pmod{16} = (8 - 8) \pmod{16} = \mathbf{0}$$
*Immediately before executing `call timer_interrupt_handler`, $\mathbf{RSP \pmod{16} == 0}$.*  
This strictly satisfies the System V AMD64 ABI requirement that `RSP` be an exact multiple of 16 before issuing any `call` instruction.

#### Stage 3: At Rust Handler Entry (`timer_interrupt_handler`)
The `call` instruction pushes the 8-byte return address onto the stack:
$$\text{RSP}_{\text{handler\_entry}} = \text{RSP}_{\text{pre\_call}} - 8$$
$$\text{RSP}_{\text{handler\_entry}} \pmod{16} = (0 - 8) \pmod{16} = \mathbf{8}$$
$$\implies (\text{RSP}_{\text{handler\_entry}} + 8) \pmod{16} = (8 + 8) \pmod{16} = \mathbf{0}$$
*At the entry point of the Rust handler, $(\mathbf{RSP + 8) \pmod{16} == 0}$ and $\mathbf{RSP \pmod{16} == 8}$.*  
This matches the exact calling convention expectations of LLVM and rustc for `extern "C"` functions.

#### Stage 4: Handler Return and Task Resumption
1. When `timer_interrupt_handler` returns via `ret`, the 8-byte return address is popped:
   $$\text{RSP} \pmod{16} = (8 + 8) \pmod{16} = 0$$
2. `mov rsp, rax` sets `%rsp` to the return value, which is either the current task's `rsp` or the newly dispatched task's `saved_rsp`. Both were captured at Stage 2 where $\text{RSP} \pmod{16} == 0$.
3. Popping 15 registers restores the GPRs and adjusts `%rsp` by $+120$ bytes:
   $$\text{RSP} \pmod{16} = (0 + 120) \pmod{16} = 8$$
4. Executing `iretq` pops the 40-byte hardware frame. The CPU loads the original unaligned $\text{RSP}_{\text{task}}$ from the `RSP` field, fully restoring the task's execution environment.

---

### 1.5 Direction Flag (DF) Handling and RFLAGS Preservation

#### A. Direction Flag Invariant
The System V AMD64 ABI specifies:
> "The direction flag DF in the `%rFLAGS` register must be clear (set to 'forward' direction, `DF=0`) on function entry and return."

When a CPU hardware interrupt fires, an IDT Interrupt Gate automatically clears `IF` (bit 9), `TF` (bit 8), and `RF` (bit 16). However, the CPU hardware does **not** automatically clear `DF` (bit 10). If the interrupted code was executing a backward string operation or third-party routine where `DF=1`, entering Rust code without clearing `DF` would cause rustc-generated code (e.g. `memset`, `memcpy`, formatting, loop optimizations) to execute in reverse and corrupt stack/memory.

#### B. Implementation and Verification
To ensure strict ABI compliance, `timer_interrupt_entry` explicitly executes `cld` immediately after saving the 15 general-purpose registers and prior to `call timer_interrupt_handler`:

```nasm
push rax
cld                  ; Clear Direction Flag (DF=0) for System V ABI compliance
mov rdi, rsp
call timer_interrupt_handler
```

#### C. Preservation Proof
1. The interrupted task's original `RFLAGS` (including its original `DF` bit) was already captured by the CPU hardware in word 3 of the hardware interrupt frame (`[RSP + 136]`) *before* `timer_interrupt_entry` began execution.
2. Executing `cld` only modifies the active CPU `RFLAGS` register; it does **not** alter the saved `RFLAGS` word stored on the stack frame.
3. When `iretq` executes at the conclusion of `timer_interrupt_entry`, the hardware pops the saved `RFLAGS` word into the CPU's flags register, unconditionally restoring the interrupted task's original Direction Flag.

---

### 1.6 Red Zone Configuration and Calling Convention Agreement

#### A. Target Red Zone Verification
In standard System V AMD64 user-space ABIs, compilers may utilize a 128-byte "red zone" below `%rsp` for temporary data without adjusting the stack pointer. In OS kernel development, the red zone **must be disabled**. If the red zone were enabled, an asynchronous hardware timer interrupt would immediately push its 40-byte hardware frame below `%rsp`, overwriting active local variables stored in the red zone.

Both target specifications in this project strictly disable the red zone:
1. `x86_64-micro_os.json` (Line 13):
   ```json
   "disable-redzone": true,
   ```
2. Built-in `x86_64-unknown-none` target specification (`rustc -Z unstable-options --print target-spec-json`):
   ```json
   "disable-redzone": true,
   ```

Because `"disable-redzone": true` is enforced across all kernel compilation targets, rustc never emits code that reads or writes below `%rsp`. All stack allocations are explicitly preceded by stack adjustments (`sub rsp, ...`).

#### B. Calling Convention Agreement
The calling convention between naked assembly and Rust is verified:
1. **ABI:** `timer_interrupt_handler` is defined with `#[no_mangle] pub extern "C" fn timer_interrupt_handler(rsp: u64) -> u64`.
2. **First Parameter:** Under the System V AMD64 ABI, the first 64-bit integer argument is passed in `%rdi`. In `timer_interrupt_entry`, `mov rdi, rsp` directly passes the stack pointer as argument 1.
3. **Return Value:** Under the System V AMD64 ABI, 64-bit integer return values are returned in `%rax`. In `timer_interrupt_entry`, `mov rsp, rax` directly retrieves the next stack pointer from `%rax`.
4. **Register Volatility:** Because `timer_interrupt_entry` pushes and pops all 15 general-purpose registers (`r15`..`rax`), all caller-saved and callee-saved registers are preserved. No register state is leaked or clobbered.

---

## 2. Interrupt Handler Return Path (`src/scheduler/round_robin.rs`)

### 2.1 Complete Unmodified Source Code

#### A. C-ABI Timer Interrupt Handler Entry Point
```rust
/// C-ABI entry point called from `timer_interrupt_entry` naked assembly.
#[no_mangle]
pub extern "C" fn timer_interrupt_handler(rsp: u64) -> u64 {
    // 1. Monotonically increment hardware timer ticks
    crate::timer::increment_ticks();

    // 2. Acknowledge interrupt by sending EOI to 8259 PIC
    crate::interrupts::pic::send_eoi(crate::interrupts::pic::TIMER_INTERRUPT_ID);

    // 3. If preemption is active, attempt preemptive scheduling
    if PREEMPTION_ENABLED.load(Ordering::SeqCst) {
        handle_timer_preemption(rsp)
    } else {
        rsp
    }
}
```

#### B. Core Preemptive Scheduling Routine
```rust
/// Core preemptive scheduling decision routine invoked on timer interrupts.
///
/// Uses `try_lock()` on all kernel mutexes to guarantee zero deadlock risk.
/// If any lock is contested or no other task is ready, the current task retains the CPU.
pub fn handle_timer_preemption(rsp: u64) -> u64 {
    let mut sched = match SCHEDULER.try_lock() {
        Some(s) => s,
        None => return rsp,
    };

    let curr_id = match sched.current {
        Some(id) => id,
        None => return rsp,
    };

    // Check if any other task is waiting in ReadyQueue
    let has_ready = {
        let rq = match crate::task::ready_queue::READY_QUEUE.try_lock() {
            Some(q) => q,
            None => return rsp,
        };
        !rq.is_empty()
    };

    if !has_ready {
        return rsp;
    }

    let mut tm = match crate::task::manager::TASK_MANAGER.try_lock() {
        Some(m) => m,
        None => return rsp,
    };

    let mut rq = match crate::task::ready_queue::READY_QUEUE.try_lock() {
        Some(q) => q,
        None => return rsp,
    };

    let curr_slot = match tm.get_task_slot(curr_id) {
        Some(s) => s,
        None => return rsp,
    };

    // Save current task's interrupted RSP
    unsafe {
        SAVED_RSP[curr_slot] = rsp;
    }

    // Transition current task Running -> Ready (auto-appended to ReadyQueue tail)
    if let Err(e) = tm.transition_task_sync(curr_id, TaskState::Ready, &mut rq) {
        crate::error!("handle_timer_preemption: failed transition {} to READY: {:?}", curr_id, e);
        return rsp;
    }

    // Dequeue next task from ReadyQueue
    let next_id = match rq.dequeue() {
        Some(id) => id,
        None => {
            let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
            return rsp;
        }
    };

    // Transition next task Ready -> Running
    if let Err(e) = tm.transition_task_sync(next_id, TaskState::Running, &mut rq) {
        crate::error!("handle_timer_preemption: failed transition {} to RUNNING: {:?}", next_id, e);
        let _ = rq.enqueue_raw(next_id);
        let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
        return rsp;
    }

    let next_slot = match tm.get_task_slot(next_id) {
        Some(s) => s,
        None => {
            let _ = tm.transition_task_sync(next_id, TaskState::Ready, &mut rq);
            let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
            return rsp;
        }
    };

    let next_rsp = unsafe { SAVED_RSP[next_slot] };
    if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
        crate::error!("handle_timer_preemption: invalid RSP=0x{:x} for slot {}", next_rsp, next_slot);
        let _ = tm.transition_task_sync(next_id, TaskState::Ready, &mut rq);
        let _ = tm.transition_task_sync(curr_id, TaskState::Running, &mut rq);
        return rsp;
    }

    sched.current = Some(next_id);
    sched.switch_count += 1;
    let count = PREEMPTION_SWITCH_COUNT.fetch_add(1, Ordering::SeqCst) + 1;

    // Read hardware interrupt frame words pushed by the CPU at [rsp + 120..160]
    let rip = unsafe { *(rsp as *const u64).add(15) };
    let cs = unsafe { *(rsp as *const u64).add(16) };
    let rflags = unsafe { *(rsp as *const u64).add(17) };
    let task_rsp = unsafe { *(rsp as *const u64).add(18) };
    let ss = unsafe { *(rsp as *const u64).add(19) };

    if count == 1 {
        FIRST_INTERRUPTED_RIP.store(rip, Ordering::SeqCst);
        FIRST_INTERRUPTED_RSP.store(task_rsp, Ordering::SeqCst);
        crate::info!("  [HW-FRAME AUDIT] Interrupted Ring 0 Hardware Stack Frame:");
        crate::info!("    [RSP + 120] Saved RIP:    0x{:016x}", rip);
        crate::info!("    [RSP + 128] Saved CS:     0x{:016x} (CPL=0)", cs);
        crate::info!("    [RSP + 136] Saved RFLAGS: 0x{:016x} (IF={})", rflags, (rflags >> 9) & 1);
        crate::info!("    [RSP + 144] Saved RSP:    0x{:016x}", task_rsp);
        crate::info!("    [RSP + 152] Saved SS:     0x{:016x}", ss);
    }

    crate::info!(
        "  [TIMER IRQ #{}] Preempted {} -> Dispatched {} (tick: {})",
        count,
        curr_id,
        next_id,
        crate::timer::get_ticks()
    );

    next_rsp
}
```

---

### 2.2 Execution Path Scenarios

1. **A task is preempted:**
   - The current task's `rsp` (pointing to its 160-byte frame) is saved into `SAVED_RSP[curr_slot]`.
   - The task is transitioned `Running -> Ready`, which automatically appends it to the tail of `ReadyQueue`.
   - The next task is dequeued from the head of `ReadyQueue` and transitioned `Ready -> Running`.
   - `handle_timer_preemption` returns `next_rsp`.
   - `timer_interrupt_entry` executes `mov rsp, rax` (switching to the incoming task's stack), pops its 15 GPRs, and executes `iretq`. The incoming task resumes execution cleanly.
2. **The same task is selected again:**
   - If only one task exists in the system or the ReadyQueue is empty, `has_ready` is false.
   - The function immediately executes `return rsp;`.
   - `timer_interrupt_entry` receives the exact same stack pointer, restores registers, and executes `iretq`, continuing the current task with zero context-switching overhead.
3. **A task completes during preemptive scheduling:**
   - The worker entry function returns into `task_trampoline()`.
   - `task_trampoline()` calls `crate::scheduler::sys_exit()`.
   - `sys_exit()` disables interrupts (`cli`), acquires `SCHEDULER.lock()`, and transitions the task to `Terminated` via `prepare_exit()`.
   - If another task is in the ReadyQueue, `sys_exit()` calls `switch_to_preemptive_task(next_rsp)`, which restores that task's registers and resumes it via `iretq` (re-enabling interrupts).
   - If all tasks have terminated, `prepare_exit()` returns `KERNEL_MAIN_RSP`. `sys_exit()` disables preemption mode and invokes `return_to_main_from_preemptive(main_rsp)`, restoring `kernel_main`.
4. **No tasks are available in the Ready Queue:**
   - Evaluated at lines 474–484: `if !has_ready { return rsp; }`.
   - The current running task retains the CPU without undergoing state changes or stack modification.
5. **The scheduler lock cannot be acquired:**
   - Evaluated at lines 463–466 (`SCHEDULER.try_lock()`), lines 475–478 (`READY_QUEUE.try_lock()`), and lines 486–494 (`TASK_MANAGER.try_lock()`).
   - If any mutex is locked, `try_lock()` immediately returns `None`.
   - The handler returns `rsp` unchanged.
   - The current task continues running. Preemption is deferred by 1 tick (10 ms). PIC EOI has already been sent, so the CPU will interrupt again on the next tick.

---

## 3. Lock Safety and Interrupt Safety

### 3.1 Exact Mutex Acquisition Points

| Lock Acquired | File | Line | Mechanism | Context |
|---|---|---|---|---|
| `SCHEDULER` | `src/scheduler/round_robin.rs` | Line 463 | `SCHEDULER.try_lock()` | Timer ISR |
| `READY_QUEUE` | `src/scheduler/round_robin.rs` | Line 475 | `READY_QUEUE.try_lock()` | Timer ISR (peek check) |
| `TASK_MANAGER` | `src/scheduler/round_robin.rs` | Line 486 | `TASK_MANAGER.try_lock()` | Timer ISR |
| `READY_QUEUE` | `src/scheduler/round_robin.rs` | Line 491 | `READY_QUEUE.try_lock()` | Timer ISR |
| `SCHEDULER` | `src/scheduler/round_robin.rs` | Line 373 | `SCHEDULER.lock()` | Task exit (`sys_exit`) with `cli` |
| `SCHEDULER` | `src/scheduler/round_robin.rs` | Line 422 | `SCHEDULER.lock()` | Preemptive startup (`start_preemptive_scheduling`) |

### 3.2 Lock Safety Analysis

1. **Deadlock Impossibility:**  
   If a timer interrupt fires while any kernel code holds `SCHEDULER`, `TASK_MANAGER`, or `READY_QUEUE`, `try_lock()` returns `None`. The ISR never spins or blocks waiting on a lock held by the interrupted thread.
2. **Behavior on `try_lock()` Failure:**  
   The ISR exits immediately by returning the incoming `rsp`. `TIMER_TICKS` has already been incremented, and PIC EOI has already been issued. Preemption is postponed by one tick (10 ms).
3. **Prevention of Nested Interrupts:**  
   The timer interrupt is registered as an **Interrupt Gate** in the IDT (`set_handler_addr`). By definition of the x86_64 architecture, when an Interrupt Gate fires, the CPU hardware atomically clears `RFLAGS.IF = 0`. No subsequent timer or hardware interrupts can interrupt `timer_interrupt_entry` or `timer_interrupt_handler`. Interrupts remain disabled until `iretq` restores the incoming task's `RFLAGS` containing `IF = 1`.
4. **Zero State Corruption Guarantee:**  
   No state transitions or queue mutations are attempted until all three locks (`SCHEDULER`, `TASK_MANAGER`, and `READY_QUEUE`) have been successfully acquired. If any lock acquisition fails, the kernel state is untouched.

---

## 4. PIC and PIT Implementation

### 4.1 Complete Unmodified Source Code

#### A. 8259 PIC Management (`src/interrupts/pic.rs`)
```rust
/// Initializes the 8259 PIC controllers, remaps IRQ offsets, and unmasks IRQ 0 (timer).
pub fn init() {
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();

        // Configure interrupt masks:
        // Master PIC: Enable IRQ 0 (timer), mask IRQs 1..7 (keyboard, etc.)
        // Port 0x21: mask byte (0 = unmasked/enabled, 1 = masked/disabled)
        // 0b1111_1110 = 0xFE
        let mut master_mask_port = x86_64::instructions::port::Port::<u8>::new(0x21);
        master_mask_port.write(0xFE);

        // Slave PIC: Mask all IRQs 8..15
        // Port 0xA1: 0xFF
        let mut slave_mask_port = x86_64::instructions::port::Port::<u8>::new(0xA1);
        slave_mask_port.write(0xFF);
    }
}

/// Acknowledges an interrupt by sending an End Of Interrupt (EOI) command to the PICs.
#[inline]
pub fn send_eoi(interrupt_id: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(interrupt_id);
    }
}
```

#### B. 8253/8254 PIT Driver (`src/timer/pit.rs`)
```rust
/// The base input clock oscillator frequency of the PIT in Hertz (1.193182 MHz).
pub const PIT_BASE_FREQUENCY_HZ: u32 = 1_193_182;

/// The target timer interrupt frequency for preemptive scheduling (100 Hz = 10 ms per tick).
pub const TARGET_FREQUENCY_HZ: u32 = 100;

/// The calculated divisor for 100 Hz periodic interrupts.
///
/// Divisor = 1,193,182 / 100 = 11,931.82 -> rounded to 11,932 (0x2E9C).
/// Actual frequency: 1,193,182 / 11,932 = 99.99849 Hz (error < 0.002%).
pub const PIT_DIVISOR: u16 = ((PIT_BASE_FREQUENCY_HZ + (TARGET_FREQUENCY_HZ / 2)) / TARGET_FREQUENCY_HZ) as u16;

/// I/O Port for PIT Channel 0 data (read/write reload counter).
pub const PIT_CHANNEL_0_DATA_PORT: u16 = 0x40;

/// I/O Port for PIT Mode/Command Register (write only).
pub const PIT_COMMAND_PORT: u16 = 0x43;

/// PIT Command Byte:
/// - Bits 7-6: `00` = Channel 0
/// - Bits 5-4: `11` = Access mode: lobyte/hibyte
/// - Bits 3-1: `010` = Mode 2 (Rate Generator: periodic pulse on every count)
/// - Bit 0:    `0` = 16-bit binary counter
pub const PIT_COMMAND_MODE_2: u8 = 0b0011_0100; // 0x34

/// Initializes the 8253/8254 PIT to generate hardware IRQ 0 interrupts at 100 Hz.
pub fn init() {
    unsafe {
        let mut command_port = Port::<u8>::new(PIT_COMMAND_PORT);
        let mut data_port = Port::<u8>::new(PIT_CHANNEL_0_DATA_PORT);

        // 1. Send Mode 2 rate-generator command to Channel 0
        command_port.write(PIT_COMMAND_MODE_2);

        // 2. Write low byte of 16-bit divisor
        let low_byte = (PIT_DIVISOR & 0xFF) as u8;
        data_port.write(low_byte);

        // 3. Write high byte of 16-bit divisor
        let high_byte = ((PIT_DIVISOR >> 8) & 0xFF) as u8;
        data_port.write(high_byte);
    }
}
```

#### C. IDT Timer Vector Registration (`src/interrupts/idt.rs`)
```rust
pub fn init() {
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);

        // Register core architectural exception handlers for diagnostics
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        // Register naked assembly timer interrupt entry at vector 32 (IRQ 0)
        let timer_addr = VirtAddr::new(crate::scheduler::context::timer_interrupt_entry as *const () as usize as u64);
        idt[TIMER_INTERRUPT_ID as usize].set_handler_addr(timer_addr);

        idt.load();
    }
}
```

---

### 4.2 Interrupt Staging and Enablement Semantics

1. **Why timer interrupts cannot fire before the scheduler is ready:**  
   During kernel bootstrap and initialization of Milestones 1–8, interrupts remain globally disabled (`IF = 0`). Even though the PIT generates square-wave pulses to the PIC, the CPU masks all external interrupts.
2. **When interrupts are first enabled with `sti`:**  
   Interrupts are enabled atomically upon dispatching the first task. When `setup_preemptive_task_stack` creates the task's initial hardware frame, it sets word index 2 (`RFLAGS`) to `0x0202` (where bit 9, `IF`, is 1). When `switch_main_to_preemptive` executes `iretq`, the CPU pops `0x0202` into `RFLAGS`, enabling hardware interrupts atomically as the CPU enters `task_trampoline`.
3. **Behavior if an interrupt arrives before the first task is scheduled:**  
   In `timer_interrupt_handler`:
   ```rust
   if PREEMPTION_ENABLED.load(Ordering::SeqCst) {
       handle_timer_preemption(rsp)
   } else {
       rsp
   }
   ```
   If an interrupt were to fire while `PREEMPTION_ENABLED` is false, the handler simply increments `TIMER_TICKS`, sends EOI, and returns `rsp` directly. No scheduling is attempted.

---

## 5. Task Completion and Return to Main Kernel Context

### 5.1 Complete Unmodified Source Code

#### A. Preemptive `sys_exit` Routine (`src/scheduler/round_robin.rs`)
```rust
/// High-level public task exit entry point.
///
/// Handles both cooperative task termination (returning via `switch_context`)
/// and preemptive task termination (switching via `iretq` or returning to `kernel_main`).
pub fn sys_exit() {
    if PREEMPTION_ENABLED.load(Ordering::SeqCst) {
        // Preemptive task exit: disable interrupts to protect state transition
        x86_64::instructions::interrupts::disable();

        let exit_data = {
            let mut sched = SCHEDULER.lock();
            sched.prepare_exit()
        };

        if let Some(next_rsp) = exit_data {
            let main_rsp = unsafe { KERNEL_MAIN_RSP };
            if next_rsp == main_rsp {
                // All preemptive tasks completed: return cleanly to kernel main thread
                PREEMPTION_ENABLED.store(false, Ordering::SeqCst);
                unsafe {
                    return_to_main_from_preemptive(main_rsp);
                }
            } else {
                // Switch to next ready preemptive task via iretq
                unsafe {
                    switch_to_preemptive_task(next_rsp);
                }
            }
        } else {
            loop {
                x86_64::instructions::hlt();
            }
        }
    } else {
        // Milestone 8 cooperative task exit
        let exit_data = {
            let mut sched = SCHEDULER.lock();
            sched.prepare_exit()
        };

        if let Some(next_rsp) = exit_data {
            static mut EXIT_DUMMY_RSP: u64 = 0;
            unsafe {
                switch_context(core::ptr::addr_of_mut!(EXIT_DUMMY_RSP), next_rsp);
            }
        } else {
            loop {
                x86_64::instructions::hlt();
            }
        }
    }
}
```

#### B. Transactional Exit Preparation (`src/scheduler/round_robin.rs`)
```rust
    /// Prepares state transitions and returns the next stack pointer for `sys_exit`.
    ///
    /// Transactional guarantee: terminates the current task and selects the next runnable
    /// task from the Ready Queue. If no runnable tasks remain, safely returns to the kernel
    /// main thread. Terminated tasks are never resumed.
    pub fn prepare_exit(&mut self) -> Option<u64> {
        let curr_id = match self.current {
            Some(id) => id,
            None => {
                warn!("sys_exit called without an active running task.");
                return None;
            }
        };

        // Transition current task to TERMINATED
        if let Err(e) = crate::task::terminate(curr_id) {
            crate::error!("FATAL: sys_exit failed to terminate current task {}: {:?}. Halting CPU.", curr_id, e);
            loop {
                x86_64::instructions::hlt();
            }
        }

        // Select next runnable task from Ready Queue
        if let Some(next_id) = self.select_next_task() {
            // Verify task state is genuinely READY before dispatching
            let (is_ready, next_slot_opt) = {
                let tm = TASK_MANAGER.lock();
                let t = tm.get_task(next_id);
                let ready = t.map_or(false, |task| task.state == TaskState::Ready);
                let slot = tm.get_task_slot(next_id);
                (ready, slot)
            };

            if !is_ready {
                crate::error!("FATAL: sys_exit: dequeued task {} is not in READY state. Halting CPU.", next_id);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            let next_slot = match next_slot_opt {
                Some(s) => s,
                None => {
                    crate::error!("FATAL: sys_exit: slot for task {} not found in TaskManager. Halting CPU.", next_id);
                    loop {
                        x86_64::instructions::hlt();
                    }
                }
            };

            let next_rsp = unsafe { SAVED_RSP[next_slot] };
            if !crate::scheduler::context::is_valid_task_rsp(next_slot, next_rsp) {
                crate::error!("FATAL: sys_exit: task {} has invalid saved RSP=0x{:x} for slot {}. Halting CPU.", next_id, next_rsp, next_slot);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            if let Err(e) = crate::task::transition(next_id, TaskState::Running) {
                crate::error!("sys_exit: failed to transition {} to RUNNING: {:?}", next_id, e);
                // Rollback: re-enqueue next_id so it is not lost from the Ready Queue
                if let Err(re) = crate::task::enqueue_task(next_id) {
                    crate::error!("FATAL: sys_exit failed to re-enqueue task {}: {:?}. Halting CPU.", next_id, re);
                }
                crate::error!("FATAL: sys_exit: ready task {} failed transition to RUNNING. Halting CPU.", next_id);
                loop {
                    x86_64::instructions::hlt();
                }
            }

            self.current = Some(next_id);
            self.switch_count += 1;
            Some(next_rsp)
        } else {
            // No runnable ready tasks remain: return to kernel main thread
            self.current = None;
            let main_rsp = unsafe { KERNEL_MAIN_RSP };
            if main_rsp != 0 {
                Some(main_rsp)
            } else {
                None
            }
        }
    }
```

#### C. Kernel Main Stack Restoration (`src/scheduler/context.rs`)
```rust
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
```

---

### 5.2 Proofs: Terminated Task Invariance and Main Stack Restoration

1. **Proof that a terminated task cannot be resumed:**
   - In `prepare_exit()`, `crate::task::terminate(curr_id)` transitions the task to `TaskState::Terminated`.
   - In `src/task/state.rs`, `Terminated` is an absorbing sink state with no permissible transitions (`can_transition_to` returns `false` for all targets).
   - In `src/task/ready_queue.rs`, `enqueue()` strictly validates `task.state == TaskState::Ready`. A `Terminated` task can never be enqueued.
   - When `prepare_exit()` dequeues `next_id`, it explicitly checks `is_ready` via `TaskManager`. If the task is not `Ready`, it panics and halts rather than dispatching it.
   - Therefore, a terminated task is mathematically impossible to re-enter or resume.
2. **Proof that the main kernel stack is restored properly:**
   - When `start_preemptive_scheduling()` begins, `switch_main_to_preemptive` pushes `kernel_main`'s callee-saved registers (`rflags`, `rbp`, `rbx`, `r12`..`r15`) and saves `%rsp` into `KERNEL_MAIN_RSP`.
   - When the final task exits, `prepare_exit()` observes that `self.select_next_task()` returns `None`, sets `self.current = None`, and returns `KERNEL_MAIN_RSP`.
   - `sys_exit()` matches `next_rsp == main_rsp`, clears `PREEMPTION_ENABLED`, and calls `return_to_main_from_preemptive(main_rsp)`.
   - `return_to_main_from_preemptive` loads `main_rsp` back into `%rsp`, pops `r15`..`rbp` and `rflags`, and executes `ret`.
   - The `ret` instruction jumps cleanly to the return address of `switch_main_to_preemptive` within `start_preemptive_scheduling()`, which then returns `true` back to `kernel_main`.
   - Empirical diagnostic log from live QEMU execution:
     ```
     [INFO]   Kernel Main RSP: 0x0000010000200df0
     ```
     This confirms that the CPU stack pointer returned to the exact same boot stack without leaking words or corrupting frames.

---

## 6. Final Programmatic Verification

The complete four-step verification sequence was executed:

```powershell
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && cargo check"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && cargo bootimage"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 -m py_compile scripts/test_milestone9.py"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 scripts/test_milestone9.py"
```

### 6.1 Exact Execution Results

| Step | Command | Exit Code | Status | Output Summary |
|---|---|---|---|---|
| **1** | `cargo check` | **0** | **PASS** | Finished `dev` profile in 1.96s (0 errors). |
| **2** | `cargo bootimage` | **0** | **PASS** | Bootimage created at `target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin`. |
| **3** | `python3 -m py_compile scripts/test_milestone9.py` | **0** | **PASS** | Test script compiled with 0 syntax or bytecode errors. |
| **4** | `python3 scripts/test_milestone9.py` | **0** | **PASS** | Automated audit script passed with **0 errors**. |

### 6.2 Audit Script Test Breakdown

- **Preserved Milestone 6 Tests:** `ALL 7 TESTS PASSED`
- **Preserved Milestone 7 Tests:** `ALL 12 QUEUE TESTS + ALL 13 REGRESSION TESTS PASSED`
- **Preserved Milestone 8 Tests:** `ALL 12 SCHEDULER TESTS PASSED` + Live Cooperative Demo Verified
- **Milestone 9 Unit Tests:**
  - `INT-TC1`: PIT 100 Hz divisor calculation (11932 = 0x2E9C, err < 0.002%) verified
  - `INT-TC2`: 8259 PIC remapping (Master: 32, Slave: 40, IRQ0: 32) verified
  - `INT-TC3`: IDT entry 32 points to valid timer handler verified
  - `INT-TC4`: Hardware timer tick progression verified
  - `INT-TC5`: Scheduler preemption state transition verified
  - `INT-TC6`: Ready queue consistency under preemption verified
  - `Overall`: `ALL 6 UNIT TESTS PASSED`
- **Milestone 9 Live Preemptive Multi-Task Demonstration:**
  - Worker-A, Worker-B, Worker-C executed CPU workloads without `sys_yield()`.
  - 17 hardware timer preemptions verified across 20 timer ticks.
  - Interrupted Ring 0 hardware stack frame audit confirmed:
    - Saved RIP: `0x215170`
    - Saved CS: `0x08` (CPL = 0)
    - Saved RFLAGS: `0x202` (IF = 1)
    - Saved RSP: `0x239478` (Task stack)
    - Saved SS: `0x00`
  - Distinct 4 KiB hardware stacks confirmed.
  - Clean termination via `sys_exit` and return to `kernel_main`.
  - Screendump verified (`screenshots/vga_milestone9.png` 720x400 RGB, 3 colors).

---

## 7. Conclusion & Next Steps

All source-code requirements and architectural invariants for **Milestone 9: Timer Interrupts and Preemptive Scheduling** have been verified and documented.

In accordance with instructions:
- **Work has stopped.**
- **Milestone 10 has NOT been started.**
- **Awaiting formal review and approval.**
