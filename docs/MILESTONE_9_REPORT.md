# Milestone 9: Timer Interrupts and Preemptive Scheduling — Implementation & Audit Report

**Course / Project**: Bare-Metal Educational Operating System in Rust (`rust-micro-os`)  
**Architecture**: x86_64 Long Mode (Bare-Metal, no standard library `#![no_std]`)  
**Status**: Completed, Fully Verified, and Audited  

---

## 1. Executive Summary

Milestone 9 implements **hardware timer interrupts** and **timer-driven preemptive Round Robin scheduling**. 

In previous milestones (Milestone 8), task switching was strictly **cooperative**: a running task had to voluntarily call `sys_yield()` to release the CPU. If a task entered an infinite loop or performed heavy compute, other tasks were completely starved.

In Milestone 9, the OS takes control of the hardware timer:
1. The **Intel 8253/8254 Programmable Interval Timer (PIT)** is configured to generate periodic interrupts at **100 Hz** (every 10 milliseconds).
2. The **dual 8259 Programmable Interrupt Controllers (PICs)** are remapped to interrupt vectors 32–47 to avoid collisions with x86 CPU architectural exceptions.
3. The **Interrupt Descriptor Table (IDT)** is loaded, registering a dedicated assembly entry point at vector 32 (IRQ 0).
4. An **asynchronous interrupt context switcher** saves the complete CPU state (15 general-purpose registers + hardware interrupt frame) and transparently preempts CPU-bound tasks.
5. Three independent worker tasks perform pure mathematical computation loops **without calling `sys_yield()`**. QEMU runtime execution proves that all three tasks are repeatedly preempted, interleaved, and resumed to completion.

All previous milestone test suites (Milestones 1–8) are 100% preserved and passing.

---

## 2. Source Code Architecture & Modified Files

```
src/
├── interrupts/                <-- [NEW SUBSYSTEM]
│   ├── mod.rs                 <-- Subsystem initializer: pic, pit, idt
│   ├── pic.rs                 <-- 8259 dual PIC driver, remapping (32/40), send_eoi()
│   ├── idt.rs                 <-- IDT registration, CPU exceptions, timer entry point
│   └── tests.rs               <-- Milestone 9 programmatic unit tests (INT-TC1..TC6)
├── timer/
│   ├── mod.rs                 <-- Atomic tick counter (TIMER_TICKS), get_ticks()
│   └── pit.rs                 <-- [NEW] 8253/8254 PIT driver, 100 Hz rate generator
├── scheduler/
│   ├── context.rs             <-- [EXPANDED] 160-byte interrupt frame, timer_interrupt_entry
│   ├── round_robin.rs         <-- [EXPANDED] handle_timer_preemption(), preemption-aware exit
│   └── tests.rs               <-- [EXPANDED] run_preemptive_task_demo() (3 CPU-bound workers)
├── main.rs                    <-- Orchestrates boot, M6/M7/M8 suites, M9 tests and demo
└── lib.rs                     <-- Added #![feature(abi_x86_interrupt)], exports interrupts
scripts/
└── test_milestone9.py         <-- Automated Python verification audit script
docs/
└── MILESTONE_9_REPORT.md      <-- This comprehensive engineering report
```

---

## 3. Interrupt Infrastructure & PIT Configuration

### A. The 8259 PIC Remapping
In the legacy IBM PC architecture, the Master PIC generated interrupt vectors `0x08..0x0F` for IRQ 0..7. In x86_64 protected and long modes, vectors `0x00..0x1F` are reserved by the Intel architecture for CPU exceptions (e.g., Vector 8 = Double Fault, Vector 13 = General Protection Fault).

If the PIC is not remapped, a hardware timer tick looks like a Double Fault CPU exception, instantly crashing the kernel!

In `src/interrupts/pic.rs`:
- **Master PIC (PIC 1)** is remapped to offset **32** (`0x20`), handling IRQs 0..7.
  - IRQ 0 (Timer) $\longrightarrow$ Vector 32 (`0x20`).
- **Slave PIC (PIC 2)** is remapped to offset **40** (`0x28`), handling IRQs 8..15.
- Interrupt masks are configured:
  - Port `0x21` (Master): `0xFE` (`0b1111_1110`): IRQ 0 enabled, IRQs 1..7 masked.
  - Port `0xA1` (Slave): `0xFF`: all slave IRQs masked.

### B. The 8253/8254 PIT Configuration (100 Hz)
The PIT operates on an internal oscillator clock of **1,193,182 Hz** (1.193182 MHz).

To generate periodic timer interrupts at 100 Hz (a 10 ms quantum):
$$\text{Divisor} = \frac{1,193,182\text{ Hz}}{100\text{ Hz}} = 11,931.82 \approx 11,932\quad (\text{hex: } 0x2E9C)$$

Actual frequency achieved:
$$\text{Frequency} = \frac{1,193,182}{11,932} = 99.99849\text{ Hz}\quad (\text{timing error } < 0.002\%)$$

In `src/timer/pit.rs`:
1. Port `0x43` (Command): Write `0x34` (`0b0011_0100`):
   - Bits 7–6 (`00`): Channel 0.
   - Bits 5–4 (`11`): Low byte followed by high byte access.
   - Bits 3–1 (`010`): Mode 2 (Rate Generator: continuous periodic pulse).
   - Bit 0 (`0`): 16-bit binary counter.
2. Port `0x40` (Data): Write low byte `0x9C` (`11932 & 0xFF`).
3. Port `0x40` (Data): Write high byte `0x2E` (`(11932 >> 8) & 0xFF`).

---

## 4. Cooperative vs. Interrupt-Driven Context Switching

Understanding the difference between cooperative and preemptive switching is crucial for college viva examinations.

| Feature | Cooperative Switching (`sys_yield`) | Interrupt-Driven Preemption (Timer IRQ) |
| :--- | :--- | :--- |
| **Trigger Mechanism** | Synchronous. The task intentionally calls a function (`sys_yield()`). | **Asynchronous**. Hardware timer fires at an arbitrary machine instruction. |
| **CPU Stack State** | Normal `call` instruction pushes `%rip` (8 bytes). | Hardware pushes **5 words (40 bytes)**: `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`. |
| **Register Preservation** | Only callee-saved registers (`rbp`, `rbx`, `r12`..`r15`) must be saved. Compiler handles caller-saved registers. | **ALL 15 general-purpose registers** (`rax`..`r15`) must be saved! Any modified register corrupts interrupted task. |
| **Return Mechanism** | `ret` instruction pops `%rip` and resumes caller. | `iretq` instruction atomically pops `RIP`, `CS`, `RFLAGS`, `RSP`, `SS`. |
| **Interrupt Flag (`IF`)** | Unchanged or manually toggled. | CPU atomically clears `IF=0` upon interrupt entry; `iretq` restores original `IF=1`. |
| **Hardware Acknowledgment**| None needed (pure software). | Must send **End Of Interrupt (EOI)** (`0x20`) to the 8259 PIC. |

### The 160-Byte Interrupt Stack Frame
When a timer interrupt interrupts a running task, the combined hardware/software frame on the stack is exactly 160 bytes:

```
Higher Memory (Top of Task Stack)
  ┌────────────────────────────────────────────────────────┐
  │ SS        (Stack Segment selector)          [RSP + 152]│ <-- Hardware pushed
  │ Old RSP   (Interrupted Stack Pointer)       [RSP + 144]│ <-- Hardware pushed
  │ RFLAGS    (CPU status flags, IF=1)          [RSP + 136]│ <-- Hardware pushed
  │ CS        (Code Segment selector, 0x08)     [RSP + 128]│ <-- Hardware pushed
  │ RIP       (Interrupted Instruction Pointer) [RSP + 120]│ <-- Hardware pushed
  ├────────────────────────────────────────────────────────┤
  │ R15       (General Purpose Register)        [RSP + 112]│ <-- Naked asm pushed
  │ R14                                         [RSP + 104]│ <-- Naked asm pushed
  │ R13                                         [RSP +  96]│ <-- Naked asm pushed
  │ R12                                         [RSP +  88]│ <-- Naked asm pushed
  │ R11                                         [RSP +  80]│ <-- Naked asm pushed
  │ R10                                         [RSP +  72]│ <-- Naked asm pushed
  │ R9                                          [RSP +  64]│ <-- Naked asm pushed
  │ R8                                          [RSP +  56]│ <-- Naked asm pushed
  │ RDI                                         [RSP +  48]│ <-- Naked asm pushed
  │ RSI                                         [RSP +  40]│ <-- Naked asm pushed
  │ RBP                                         [RSP +  32]│ <-- Naked asm pushed
  │ RBX                                         [RSP +  24]│ <-- Naked asm pushed
  │ RDX                                         [RSP +  16]│ <-- Naked asm pushed
  │ RCX                                         [RSP +   8]│ <-- Naked asm pushed
  │ RAX                                         [RSP +   0]│ <-- Naked asm pushed
  └────────────────────────────────────────────────────────┘
Lower Memory (Current RSP pointing to RAX)
```
- **Alignment Proof**: $160\text{ bytes} = 10 \times 16\text{ bytes}$. The stack maintains 16-byte alignment as required by the System V AMD64 ABI.

---

## 5. Lock Safety & Deadlock Prevention Strategy

### The Same-Core Interrupt Deadlock Problem
In a single-core OS, suppose task code holds a kernel spinlock (e.g., `TASK_MANAGER.lock()`). If a timer interrupt fires at that exact microsecond:
1. The CPU jumps into `timer_interrupt_handler`.
2. If `timer_interrupt_handler` attempts to acquire `TASK_MANAGER.lock()`, it will spin in an infinite loop waiting for the lock to be released.
3. But the interrupted task cannot release the lock because the CPU is occupied spinning in the interrupt handler!
4. **Result**: Permanent system freeze (Deadlock).

### Our Dual-Layer Defense
1. **Defensive Non-Blocking Locking (`try_lock`)**:
   Inside `handle_timer_preemption()`:
   ```rust
   let mut sched = match SCHEDULER.try_lock() {
       Some(s) => s,
       None => return rsp, // Lock held by interrupted code! Skip preemption this tick.
   };
   ```
   If ANY lock (`SCHEDULER`, `TASK_MANAGER`, or `READY_QUEUE`) is contested, the handler never spins! It immediately returns `rsp`, incrementing `TIMER_TICKS` and acknowledging the PIC. The task is simply preempted on the subsequent tick 10ms later.
2. **Interrupt Masking in Task Management**:
   Whenever kernel code accesses mutexes outside an interrupt, it runs with interrupts disabled (`x86_64::instructions::interrupts::without_interrupts`). Because interrupts cannot fire while a lock is held, lock contention inside the handler is virtually eliminated.

---

## 6. Verification & Test Results

### A. Milestone 9 Programmatic Unit Tests (6/6 Passed)
```text
[INFO] ========================================================
[INFO]    RUNNING MILESTONE 9: INTERRUPT & TIMER UNIT TESTS    
[INFO] ========================================================
[INFO]   [PASS] INT-TC1: PIT 100 Hz divisor calculation (11932 = 0x2E9C, err < 0.002%) verified.
[INFO]   [PASS] INT-TC2: 8259 PIC remapping (Master: 32, Slave: 40, IRQ0: 32) verified.
[INFO]   [PASS] INT-TC3: IDT entry 32 points to valid timer handler at 0x214c10.
[INFO]   [PASS] INT-TC4: Hardware timer tick progression (0 -> 3) verified.
[INFO]   [PASS] INT-TC5: Scheduler preemption state transition (Running->Ready at tail) verified.
[INFO]   [PASS] INT-TC6: Ready queue consistency under preemption (blocked/term excluded) verified.
[INFO] >>> MILESTONE 9 INTERRUPT TESTS: ALL 6 UNIT TESTS PASSED <<<
```

### B. Live Preemptive Multi-Task Demonstration Evidence
Three independent worker tasks (`Preempt-A`, `Preempt-B`, `Preempt-C`) were executed with CPU-bound workloads. **None of the workers called `sys_yield()`.**

Runtime Serial Trace:
```text
[INFO] --- BEGIN DEMO: Live Preemptive Round Robin Execution (100 Hz Timer) ---
[INFO] Demo Setup: Registered preemptive tasks T100, T101, T102.
[INFO] Demo Setup: Enqueued tasks in FIFO order: T100 -> T101 -> T102.
[INFO] Starting timer-driven preemptive Round Robin scheduler...
[INFO]   [TIMER IRQ #1] Preempted T100 -> Dispatched T101 (tick: 4)
[INFO]   [Preempt Worker-B] Started CPU-bound computation.
[INFO]   [TIMER IRQ #2] Preempted T101 -> Dispatched T102 (tick: 5)
[INFO]   [Preempt Worker-C] Started CPU-bound computation.
[INFO]   [TIMER IRQ #3] Preempted T102 -> Dispatched T100 (tick: 6)
[INFO]   [Preempt Worker-A] Started CPU-bound computation.
[INFO]   [Preempt Worker-A] Phase 1/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #4] Preempted T100 -> Dispatched T101 (tick: 7)
[INFO]   [Preempt Worker-B] Phase 1/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #5] Preempted T101 -> Dispatched T102 (tick: 8)
[INFO]   [Preempt Worker-C] Phase 1/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #6] Preempted T102 -> Dispatched T100 (tick: 9)
[INFO]   [TIMER IRQ #7] Preempted T100 -> Dispatched T101 (tick: 10)
[INFO]   [Preempt Worker-B] Phase 2/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #8] Preempted T101 -> Dispatched T102 (tick: 11)
[INFO]   [Preempt Worker-C] Phase 2/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #9] Preempted T102 -> Dispatched T100 (tick: 12)
[INFO]   [Preempt Worker-A] Phase 2/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #10] Preempted T100 -> Dispatched T101 (tick: 13)
[INFO]   [Preempt Worker-B] Phase 3/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #11] Preempted T101 -> Dispatched T102 (tick: 14)
[INFO]   [Preempt Worker-C] Phase 3/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #12] Preempted T102 -> Dispatched T100 (tick: 15)
[INFO]   [Preempt Worker-A] Phase 3/3 executing CPU workload (no sys_yield)...
[INFO]   [TIMER IRQ #13] Preempted T100 -> Dispatched T101 (tick: 16)
[INFO]   [Preempt Worker-B] Completed all 3 compute phases - exiting.
[INFO]   [Preempt Worker-C] Completed all 3 compute phases - exiting.
[INFO]   [TIMER IRQ #14] Preempted T102 -> Dispatched T100 (tick: 17)
[INFO]   [Preempt Worker-A] Completed all 3 compute phases - exiting.
[INFO] Returned to kernel main thread! Verifying preemptive execution...
[INFO] Demo Success: All 3 workers completed workloads (A: 3/3, B: 3/3, C: 3/3).
[INFO] Hardware timer preemption confirmed: 15 context switches across 18 ticks.
[INFO] Hardware Stack Diagnostic Evidence (Preemptive Tasks):
[INFO]   Kernel Main RSP: 0x0000010000200e50
[INFO]   Preempt-A   RSP: 0x0000000000238e20 (Slot 10 buffer: 0x237eb0 - 0x238eb0)
[INFO]   Preempt-B   RSP: 0x0000000000239e20 (Slot 11 buffer: 0x238eb0 - 0x239eb0)
[INFO]   Preempt-C   RSP: 0x000000000023ae20 (Slot 12 buffer: 0x239eb0 - 0x23aeb0)
[INFO] Stack Diagnostic: PASS - Confirmed distinct, isolated 4 KiB stacks during timer interrupts.
[INFO] Live timer-driven preemptive execution demonstration verified successfully.
```

### C. Automated Script Conformance (`scripts/test_milestone9.py`)
- Python script compiled and executed with **exit code 0**.
- **0 errors** across all conformance assertions.
- Verified QEMU did not exit prematurely.
- VGA Framebuffer screendump captured, converted to PNG, and verified at 720x400 with 3 active color codes.

---

## 7. Visual Artifact
- Local repository path: `screenshots/vga_milestone9.png`
- Conversation artifact link: [vga_milestone9.png](file:///C:/Users/roshi/.gemini/antigravity/brain/7557e3cf-60c1-4710-bc94-8a4900f862d6/vga_milestone9.png)

---

## 8. College Viva Q&A Guide

**Q1: Why must the 8259 PIC be remapped before enabling interrupts in x86 Protected/Long Mode?**  
*Answer*: By default, the IBM PC maps IRQs 0–7 to interrupt vectors `0x08..0x0F`. In x86_64, these vectors are reserved for CPU exceptions (e.g., Vector 8 is Double Fault). Remapping the Master PIC to vector 32 and Slave to 40 moves hardware IRQs above vector 31, preventing hardware interrupts from being misinterpreted as CPU faults.

**Q2: What is the purpose of the End Of Interrupt (EOI) signal?**  
*Answer*: The 8259 PIC prioritizes interrupts. When an IRQ is triggered, the PIC sets the corresponding In-Service Register (ISR) bit and stops delivering equal- or lower-priority interrupts until software sends an EOI byte (`0x20` to port `0x20`). Without EOI, only a single timer tick would ever be delivered.

**Q3: Why can't a cooperative `switch_context` function simply be called inside a hardware interrupt?**  
*Answer*: A cooperative switch assumes a function call convention where caller-saved registers can be clobbered and returns via `ret`. A hardware interrupt fires asynchronously: the interrupted task could be in the middle of using any general-purpose register. All 15 GPRs must be preserved, and the CPU must return via `iretq` to atomically restore the hardware interrupt frame (`SS, RSP, RFLAGS, CS, RIP`).

**Q4: How do you prevent deadlocks when acquiring spinlocks inside a timer interrupt handler?**  
*Answer*: We use non-blocking `try_lock()`. If a mutex is held by the interrupted code, the handler does not spin; it sends the PIC EOI, increments ticks, and immediately returns to let the interrupted code complete and release the lock. Furthermore, kernel critical sections disable interrupts (`without_interrupts`).

---

## 9. Conclusion
Milestone 9 is completely implemented, verified, and audited. The kernel exhibits genuine hardware timer interrupts, atomic tick progression, and preemptive scheduling across independent CPU-bound tasks.

**Milestone 10 has not been started. I await your review and formal approval.**
