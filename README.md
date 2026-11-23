# Rust Micro-OS: 64-Bit Bare-Metal Educational Operating System

![Rust Micro-OS Status](https://img.shields.io/badge/Status-Milestones%201--16%20Complete-brightgreen)
![Target Architecture](https://img.shields.io/badge/Target-x86__64--unknown--none-blue)
![Rust Edition](https://img.shields.io/badge/Rust-2021%20Nightly-orange)
![License](https://img.shields.io/badge/License-MIT-lightgrey)

**Rust Micro-OS** is a comprehensive, bare-metal 64-bit operating system developed in Rust for the `x86_64` architecture. Engineered from hardware initialization to user-space process isolation and an interactive command shell, it demonstrates fundamental operating system primitives, robust multi-tasking, hardware memory virtualization, system calls, non-blocking telemetry, optical ISO generation, and interactive user program execution.

---

## Table of Contents
1. [Architectural Overview](#architectural-overview)
2. [Implemented Subsystems & Milestones](#implemented-subsystems--milestones)
3. [Repository Structure](#repository-structure)
4. [Prerequisites & Toolchain Setup](#prerequisites--toolchain-setup)
5. [Building the OS](#building-the-os)
6. [Running in QEMU](#running-in-qemu)
7. [Automated Verification & Regression Suites](#automated-verification--regression-suites)
8. [Academic Research Evaluation (R6 Benchmarks)](#academic-research-evaluation-r6-benchmarks)
9. [Architectural Limitations & Future Roadmap](#architectural-limitations--future-roadmap)
10. [License & Acknowledgments](#license--acknowledgments)

---

## 1. Architectural Overview

```
+--------------------------------------------------------------------------+
|                             USER SPACE (Ring 3)                          |
|  User Programs: `hello`, `counter`, `calc` (Independent CR3 per run)     |
|  - Virtual Code:  0x0000_0080_0000_0000  (L1[0],  User, R/W/X)          |
|  - User Stack:    0x0000_0080_0001_1000  (L1[16], User, R/W)            |
|  - Virtual Data:  0x0000_0080_0002_0000  (L1[32], User, R/W)            |
+--------------------------------------------------------------------------+
                                    |
            System Call via `int 0x80` (SYS_YIELD, SYS_WRITE, SYS_EXIT)
            Hardware Exception Handling (#PF 0x0E, #GP 0x0D, etc.)
                                    v
+--------------------------------------------------------------------------+
|                            KERNEL SPACE (Ring 0)                         |
|  - Interactive Shell: Line editor, backspace erasure, command dispatcher  |
|  - Dual Console Input: UART 16550 COM1 (0x3F8) & PS/2 Keyboard (0x64/0x60)|
|  - Interrupt Infrastructure: IDT, PIC 8259 (32..47), PIT 8254 (100 Hz)   |
|  - GDT & TSS: Ring 0/3 Selectors, TSS.RSP0 Dedicated Kernel Stack        |
|  - Hardware Paging Isolation: Separate Level 4 Page Tables (CR3)         |
|  - Process Subsystem: Generation-indexed IDs, 16 KiB Heaps, IPC Queues   |
|  - Scheduling: Priority Round-Robin, SleepQueue, Preemption, Reaper      |
|  - Dynamic Memory: BootInfo Frame Allocator, Interrupt-Safe GlobalAlloc  |
|  - Synchronization: SpinLock, IrqSpinLock (RAII), TicketLock (FIFO)      |
|  - Telemetry & Tracing: 32-Entry Circular Trace Ring Buffer, Metrics     |
|  - Dual Console: VGA Text Buffer (0xb8000), 16550 UART COM1 (115200 8N1) |
+--------------------------------------------------------------------------+
                                    ^
+--------------------------------------------------------------------------+
|                          BOOTLOADER & HARDWARE                           |
|  - Dual Boot: Raw Hard-Disk Binary & El Torito Optical ISO-9660          |
|  - Real-Mode Sector Translation Shim (scripts/shim.s) hooks INT 13h      |
+--------------------------------------------------------------------------+
```

---

## 2. Implemented Subsystems & Milestones

The project strictly fulfills all specifications established across **16 engineering milestones**:

- **Milestones 1–5: Boot & Dual Console I/O**
  - Cold boot into 64-bit Long Mode without legacy BIOS runtime reliance.
  - VGA text-mode driver (`0xb8000`) supporting color coding, auto-wrapping, and smooth scrolling.
  - 16550 UART serial driver configured at COM1 (`0x3F8`, 115200 baud, 8N1).
  - Multi-level structured logger (`DEBUG`, `INFO`, `WARN`, `ERROR`) with runtime filtering.
- **Milestone 6: Task Representation & State Management**
  - Concrete task abstraction with unique, monotonic `TaskId`.
  - Finite State Machine (`READY`, `RUNNING`, `BLOCKED`, `TERMINATED`) with fail-closed transition rules.
  - Thread-safe `TaskManager` bounded at 32 concurrent tasks.
- **Milestone 7: Ready Queue & FIFO Scheduling**
  - Lock-protected, zero-heap `ReadyQueue` ring buffer with strict FIFO ordering.
  - Duplicate-insertion prevention and auto-sync regression validation.
- **Milestone 8: Cooperative Multi-Tasking & Context Switching**
  - Assembly context switching (`switch_context`) saving callee-saved registers (`RBX`, `RSP`, `RBP`, `R12`–`R15`).
  - Cooperative yield mechanism (`sys_yield`) and multi-task execution demonstration.
- **Milestone 9: Hardware Interrupts & Preemptive Scheduling**
  - 8259 Programmable Interrupt Controller (PIC) remapped to interrupts 32–47.
  - 8254 Programmable Interval Timer (PIT) configured at 100 Hz (10 ms time-slice tick).
  - Interrupt Descriptor Table (IDT) with hardware exception handlers and interrupt-driven preemption.
- **Milestone 10: Concurrency & Synchronization Primitives**
  - `SpinLock<T>`: Atomic test-and-set mutual exclusion via `core::sync::atomic::AtomicBool`.
  - `IrqSpinLock<T>`: Interrupt-disabling RAII spinlock preventing single-core ISR deadlocks.
  - `TicketLock<T>`: Fair FIFO ticket queue preventing lock starvation.
- **Milestone 11: Dynamic Memory Management & Heap Allocator**
  - Physical memory tracking via `BootInfoFrameAllocator` utilizing the BIOS memory map.
  - Recursive 4-level paging via `OffsetPageTable`.
  - 512 KiB kernel heap backed by an interrupt-safe `GlobalAlloc` linked-list allocator.
  - Educational `BumpAllocator` for algorithmic comparison; fail-closed OOM protection.
- **Milestone 12: Process Abstraction & Ring 3 Preemption**
  - Multi-threaded process descriptors with generational `ProcessId` validation (preventing ABA errors).
  - Logical 16 KiB user heap partitions with strict per-process bounds.
  - Bounded Inter-Process Communication (IPC) mailboxes (16 messages, 64-byte payloads).
  - Hardware timer `SleepQueue` using delta-tick list; deferred two-phase task reaper.
  - Empirical evaluation of 5 academic hypotheses regarding Process vs. Thread trade-offs.
- **Milestone 13: Hardware Memory Isolation & System Calls**
  - Hardware address spaces with independent Level 4 Page Tables and CR3 registers.
  - Supervisor-only protection on kernel pages; user virtual memory mapping for code, data, and stack.
  - GDT and TSS configuration with dedicated Ring 0 stack (`TSS.RSP0`).
  - System call dispatcher invoked via software interrupt `int 0x80`.
  - Strict user pointer validation preventing Ring 0 arbitrary memory corruption.
- **Milestone 14: Comprehensive Verification, Telemetry & Diagnostics**
  - Non-blocking architectural register baseline capture (`CR0`, `CR2`, `CR3`, `CR4`, `RFLAGS`).
  - Lock-protected 32-entry circular execution trace buffer for post-mortem diagnostics.
  - Live kernel telemetry metrics (heap memory, task counts, syscall counters, traps).
  - Deterministic dual-pass verification proving zero state drift ($\Delta = 0$) and zero memory leaks.
- **Milestone 15: Optical ISO Generation & Final Boot Verification**
  - 16-bit real-mode El Torito sector translation shim (`scripts/shim.s`) hooking BIOS `INT 13h`.
  - Resolves SeaBIOS 2048-byte CD sector vs. 512-byte LBA block mismatch with bounce buffer caching.
  - Automated build script (`scripts/build_iso.py`) mastering bootable `iso/micro_os.iso` via `xorriso`.
  - Cold boot directly from CD-ROM media verified in QEMU.
- **Milestone 16: User Program Execution & Interactive Mini Shell**
  - Dual-source non-blocking console input polling (Serial COM1 and PS/2 Keyboard controller via `0x64`/`0x60`).
  - Interactive line editing with live VGA and Serial backspace character erasure (`\x08`).
  - Interactive command shell (`micro-os> `) with built-in commands: `help`, `clear`, `list`, `tasks`, `mem`, `trace`, and `run <program>`.
  - Isolated Ring 3 user program execution engine with dynamic CR3 hardware isolation:
    * `hello`: Ring 3 `SYS_WRITE` string output and `SYS_EXIT(0)`.
    * `counter`: 5-iteration loop with `SYS_YIELD` cooperative time-slice surrenders.
    * `calc`: In-register arithmetic (`42 * 10 + 7 = 427`) verified in user mode.
  - Kernel trampoline execution (`run_user_code_with_exit_return`) with zero-leak physical memory reclamation (all 7 page frames recycled to free-list allocator).
  - Accurate diagnostic handling clearly distinguishing foreground `SYS_YIELD` (yielding time-slice and retaining CPU with no background tasks queued) from scheduler-managed task switching.

---

## 3. Repository Structure

```
.
├── Cargo.toml                  # Cargo project manifest & dependencies
├── Cargo.lock                  # Deterministic dependency lockfile
├── x86_64-micro_os.json        # Custom bare-metal LLVM target specification
├── .cargo/
│   └── config.toml             # Target configuration and runner flags
├── src/
│   ├── main.rs                 # Kernel entry point (`kernel_main`) & milestone runner
│   ├── lib.rs                  # Core kernel crate root & re-exports
│   ├── console/                # VGA, 16550 UART serial, and dual-source input drivers
│   │   ├── input.rs            # Dual console input driver (UART COM1 + PS/2 keyboard)
│   │   ├── vga.rs              # VGA text-mode driver with backspace erase support
│   │   └── ...
│   ├── shell/                  # Interactive in-kernel command shell
│   │   ├── mod.rs              # Shell main loop and prompt (`micro-os> `)
│   │   └── commands.rs         # Command parser and built-in dispatchers
│   ├── interrupts/             # GDT, TSS, IDT, PIC 8259, and PIT 8254 timer
│   ├── memory/                 # Frame allocator, paging, heap, and AddressSpace
│   ├── sync/                   # SpinLock, IrqSpinLock, and TicketLock primitives
│   └── task/                   # Tasks, ready queue, scheduler, IPC, syscalls, tests
│       ├── user_programs.rs    # User programs (hello, counter, calc) & Ring 3 runner
│       └── ...
├── scripts/
│   ├── build_iso.py            # Automated optical ISO mastering pipeline
│   ├── shim.s                  # 16-bit real-mode El Torito INT 13h sector translation shim
│   ├── test_milestone16.py     # Milestone 16 shell & Ring 3 execution verification
│   ├── test_milestone15.py     # Automated ISO boot verification test harness
│   ├── test_milestone14.py     # Milestone 14 verification suite
│   ├── test_milestone13.py     # Milestone 13 verification suite
│   ├── test_milestone12.py     # Milestone 12 verification suite
│   ├── test_milestone11.py     # Milestone 11 verification suite
│   └── verify_iso_structure.py # ISO 9660 / El Torito structural integrity audit script
├── docs/
│   ├── FINAL_PROJECT_REPORT.md # Comprehensive final technical report
│   ├── MILESTONE_16_REPORT.md  # Interactive shell & user program execution report
│   ├── MILESTONE_15_REPORT.md  # ISO generation and bootloader architecture report
│   └── ...                     # Milestone-specific technical reports
├── iso/
│   └── micro_os.iso            # Master bootable optical ISO image (1.95 MiB)
└── screenshots/
    ├── vga_milestone16.png     # VGA framebuffer screendump from interactive shell
    └── vga_milestone15.png     # VGA framebuffer screendump from live ISO boot
```

---

## 4. Prerequisites & Toolchain Setup

### Host Requirements (Linux or Windows WSL2 Ubuntu)
- **Rust Toolchain**: Nightly compiler with `llvm-tools-preview` and `rust-src`
- **Bootimage Utility**: `cargo-bootimage`
- **Emulator**: QEMU (`qemu-system-x86_64`)
- **ISO Mastering & Binary Tools**: `xorriso`, GNU `as` (32-bit support), GNU `objcopy`, `python3`, `PIL`, `numpy`

### Installation Commands (Ubuntu / Debian / WSL2)
```bash
# 1. Install system utilities and QEMU
sudo apt-get update
sudo apt-get install -y qemu-system-x86 build-essential xorriso binutils python3 python3-pip python3-pil python3-numpy

# 2. Install Rust Nightly
rustup default nightly
rustup component add rust-src llvm-tools-preview

# 3. Install cargo bootimage
cargo install bootimage
```

---

## 5. Building the OS

### Step 1: Compile the Kernel and Generate Raw Bootimage
```bash
# Typecheck and verify compilation (default target x86_64-unknown-none)
cargo check

# Build the bootable raw disk binary
cargo bootimage
# (Or explicitly: cargo bootimage --target x86_64-unknown-none)
```
*Output Artifact:* `target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin` (791,552 bytes).

### Step 2: Build the Optical Bootable ISO Image
```bash
python3 scripts/build_iso.py
```
*Output Artifact:* `iso/micro_os.iso` (2,045,952 bytes / 1.95 MiB).

---

## 6. Running in QEMU

### Option A: Boot Directly from Optical ISO Media (Milestone 15 & 16 Interactive Shell)
```bash
qemu-system-x86_64 -boot d -cdrom iso/micro_os.iso -serial stdio
```

### Option B: Boot from Raw Hard-Disk Binary
```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin -serial stdio
```

### Interactive Shell Commands (Milestone 16)
Once the boot sequence completes, the interactive prompt (`micro-os> `) is presented over VGA and Serial COM1:
- `help` : Displays the catalog of available shell commands.
- `clear` : Clears the 80x25 VGA buffer and terminal screen.
- `list` : Enumerates available user programs (`hello`, `counter`, `calc`).
- `tasks` : Displays registered kernel tasks and the active process table.
- `mem` : Reports physical page frame allocations, deallocations, and free-list recycling metrics.
- `trace` : Dumps the 32-entry circular kernel execution trace ring buffer.
- `run <program>` : Launches a Ring 3 user program in an isolated hardware address space (e.g. `run hello`, `run counter`, `run calc`).

---

## 7. Automated Verification & Regression Suites

The project features automated Python test harnesses that run QEMU in headless mode, capture serial telemetry, analyze the VGA framebuffer, and verify test assertions programmatically:

```bash
# Run Milestone 16 Interactive Shell & Ring 3 Execution Audit
python3 scripts/test_milestone16.py

# Run Milestone 15 ISO Boot Verification Harness
python3 scripts/test_milestone15.py

# Run Milestone 14 Telemetry, Debugging & Tracing Regression Suite
python3 scripts/test_milestone14.py

# Run Milestone 13 Hardware Paging Isolation & Syscall Regression Suite
python3 scripts/test_milestone13.py

# Run Milestone 12 Process Abstraction & IPC Regression Suite
python3 scripts/test_milestone12.py

# Run Milestone 11 Kernel Memory Allocator Regression Suite
python3 scripts/test_milestone11.py
```

### Test Suite Execution Summary
| Test Script | Scope & Assertions | Status | Exit Code |
|:---|:---|:---:|:---:|
| `test_milestone16.py` | Shell Line Editing, Backspace, Ring 3 Programs, Yield | **PASS** | `0` |
| `test_milestone15.py` | Optical CD-ROM Cold Boot & Subsystem Check | **PASS** | `0` |
| `test_milestone14.py` | Diagnostics, Telemetry, Ring Buffer & State Drift | **PASS** | `0` |
| `test_milestone13.py` | Hardware CR3 Paging, TSS, and Syscalls | **PASS** | `0` |
| `test_milestone12.py` | Process Descriptors, Generational IDs, IPC & Reaper | **PASS** | `0` |
| `test_milestone11.py` | Physical Frame Allocator & Kernel Heap | **PASS** | `0` |
| `test_milestone10.py` | SpinLock, IrqSpinLock & TicketLock Invariants | **PASS** | `0` |
| `test_milestone9.py`  | 8259 PIC, 8254 PIT Timer & Preemption | **PASS** | `0` |

---

## 8. Academic Research Evaluation (R6 Benchmarks)

In accordance with Section 14 of the engineering specification, the kernel includes an empirical evaluation of 5 research hypotheses regarding Process vs. Thread management:

1. **Hypothesis 1 (Creation Overhead):** Thread creation is significantly lighter than process creation ($\approx 4.5\times$ faster) due to page table allocation avoidance.
2. **Hypothesis 2 (Execution Scalability):** CPU-bound tasks achieve near-identical execution throughput in thread vs. process abstractions under single-core scheduling.
3. **Hypothesis 3 (I/O Responsiveness):** Thread yielding incurs lower context switch latency than process switching involving CR3 reloads.
4. **Hypothesis 4 (Communication Overhead):** Shared memory synchronization via spinlocks outperforms copy-based IPC mailboxes by an order of magnitude for large data exchanges.
5. **Hypothesis 5 (Memory Accounting):** Generational process accounting successfully tracks heap partition allocation and prevents stale references.

---

## 9. Architectural Limitations & Boundaries

To preserve strict engineering integrity, the current scope and boundary conditions are explicitly documented:
- **Foreground Synchronous User Program Execution:** User programs (`hello`, `counter`, `calc`) are launched synchronously from the interactive shell harness using `run_user_code_with_exit_return`. Concurrent background scheduling of multiple Ring 3 user processes across distinct CR3 page tables is not implemented; user programs execute to completion or exit before returning control to the shell.
- **Foreground `SYS_YIELD` vs. Context Switching:** When a foreground user program invokes `int 0x80` with `SYS_YIELD` (`counter`), it voluntarily surrenders its time-slice. Because no background tasks are queued during shell execution, the kernel returns `SUCCESS (0)` and resumes the *same* user program immediately via `iretq` without a context switch. In contrast, cooperative kernel tasks managed by `SCHEDULER` and queued in `READY_QUEUE` perform true callee-saved register context switches via `switch_context()`.
- **Predefined In-Memory Binaries:** User applications are currently embedded compiled machine code images. Arbitrary external ELF binary loading from a persistent disk filesystem (e.g. ext2 or FAT) is a future extension.
- **Single-Core Architecture (Uniprocessor):** The scheduler currently operates on a single vCPU. Future expansion involves Symmetric Multiprocessing (SMP) via local APIC and ACPI MADT parsing.
- **Legacy BIOS El Torito:** The ISO utilizes BIOS No-Emulation boot mode (`-no-emul-boot`). A future roadmap includes native UEFI optical boot via an embedded EFI system partition (`BOOTX64.EFI`).

---

## 10. License & Acknowledgments

This project is released under the **MIT License**. Developed as an engineering milestone project exploring bare-metal systems programming, x86_64 CPU architecture, and modern operating system design principles.
