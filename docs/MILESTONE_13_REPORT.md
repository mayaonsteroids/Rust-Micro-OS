# Milestone 13 Technical Verification Report
## User Mode, Hardware Paging Isolation, and System Calls

**Project:** Rust Micro-OS (64-bit Long Mode Bare-Metal Kernel)  
**Target:** `x86_64-unknown-none`  
**Milestone:** 13 (User Mode, Hardware Paging Isolation, and System Calls)  
**Status:** COMPLETED & VERIFIED (Zero Regressions on Milestones 1–12)  
**Date:** 2026-11-23  

---

## 1. Executive Summary

Milestone 13 introduces **true hardware-enforced memory isolation**, **Ring 3 privilege separation**, and an **architectural system call interface** via software interrupt Vector `0x80`.

Prior to Milestone 13, all tasks and processes ran within Ring 0 (Kernel Mode) sharing the bootloader's active page table. In Milestone 13:
1. **Hardware Memory Isolation**: Every process is provisioned with its own distinct Level 4 Page Table (`cr3: PhysFrame`). The kernel space is mirrored with supervisor-only permissions (`!USER_ACCESSIBLE`), while the user space (`0x0000_0080_0000_0000` .. `0x0000_0080_0003_0000`) is mapped with `PRESENT | WRITABLE | USER_ACCESSIBLE`.
2. **Ring 3 Privilege Transition**: The Global Descriptor Table (GDT) and Task State Segment (TSS) are initialized with 64-bit User Code (Selector `0x23`, RPL=3), User Data (Selector `0x1b`, RPL=3), and a dedicated Ring 0 privilege stack (`TSS.privilege_stack_table[0]`). Privilege transitions from kernel to user mode are executed via `iretq`.
3. **System Call Interface (`int 0x80`)**: User-mode applications request kernel services via software interrupt Vector `0x80`. The interrupt gate automatically switches the CPU stack pointer to `TSS.RSP0`, preserves user registers, validates user-supplied pointers, and dispatches to kernel routines (`SYS_WRITE`, `SYS_YIELD`, `SYS_EXIT`).
4. **Scheduler Integration**: Both voluntary yielding (`sys_yield`) and preemptive timer interrupts (`timer_interrupt_handler`) transparently switch CR3 and update `TSS.RSP0` when context-switching to/from user processes, while preserving 100% compatibility for kernel threads.

---

## 2. Architecture & Implementation Details

### 2.1 Hardware Memory Isolation & Address Space Hierarchy

The virtual address space is organized along 512 GiB Level 4 Page Table boundaries:

| Region | Virtual Address Range | L4 Index | Page Table Flags | Privilege Level |
|---|---|---|---|---|
| **User Code** | `0x0000_0080_0000_0000` | 1 | `PRESENT \| WRITABLE \| USER_ACCESSIBLE` | Ring 3 (User) |
| **User Stack** | `0x0000_0080_0001_0000` | 1 | `PRESENT \| WRITABLE \| USER_ACCESSIBLE` | Ring 3 (User) |
| **User Data** | `0x0000_0080_0002_0000` | 1 | `PRESENT \| WRITABLE \| USER_ACCESSIBLE` | Ring 3 (User) |
| **Kernel Heap** | `0x0000_4444_4444_0000` | 136 | `PRESENT \| WRITABLE` | Ring 0 (Supervisor) |
| **Kernel Low Memory / VGA** | `0x0000_0000_000b_8000` | 0 | `PRESENT \| WRITABLE` | Ring 0 (Supervisor) |
| **Physical Memory Offset** | Bootloader mapping | 256..511 | `PRESENT \| WRITABLE` | Ring 0 (Supervisor) |

#### Physical Frame Ownership & Recycling
- `GlobalFrameAllocator` manages physical 4 KiB frames using an internal free list (`freed_frames: [Option<PhysFrame>; 128]`).
- When an `AddressSpace` is created, it allocates 7 physical frames:
  - 4 page table frames: L4 table, L3 table, L2 table, L1 table.
  - 3 memory backing frames: User Code page, User Stack page, User Data page.
- Upon `AddressSpace::destroy(self)`, all 7 frames are reclaimed into the free list for immediate reuse without memory leaks.

### 2.2 Global Descriptor Table (GDT) and Task State Segment (TSS)

The GDT is loaded with 5 descriptors conforming to System V AMD64 ABI:
- **Null Descriptor** (Index 0, Selector `0x00`)
- **Kernel Code Segment** (Index 1, DPL=0, Selector `0x08`)
- **Kernel Data Segment** (Index 2, DPL=0, Selector `0x10`)
- **User Data Segment** (Index 3, DPL=3, Selector `0x1b`, RPL=3)
- **User Code Segment** (Index 4, DPL=3, Selector `0x23`, RPL=3)
- **Task State Segment (TSS)** (Index 5–6, System Descriptor, Selector `0x28`)

`TSS.privilege_stack_table[0]` (`RSP0`) is configured with the active task's kernel stack pointer, ensuring that hardware interrupts or system calls from Ring 3 land on a valid kernel stack.

### 2.3 System Call Interface & Pointer Validation

System calls use software interrupt Vector `0x80` (`IDT[0x80]` with DPL=3):
- **ABI Calling Convention**:
  - `%rax`: Syscall Number (`SYS_WRITE = 1`, `SYS_YIELD = 2`, `SYS_EXIT = 3`)
  - `%rdi`: Arg 1 (Buffer pointer or exit code)
  - `%rsi`: Arg 2 (Buffer length)
  - `%rdx`: Arg 3 (Auxiliary argument)
  - Return value returned in `%rax`.
- **Pointer Validation**:
  - `is_valid_user_range(ptr, len)` strictly verifies that `ptr >= 0x0000_0080_0000_0000` and `ptr + len <= 0x0000_0080_0003_0000`.
  - Null pointers, canonical kernel higher-half pointers, kernel heap pointers, and overflowing addresses are rejected with `-14` (`EFAULT`).
  - Unimplemented syscall numbers are rejected with `-38` (`ENOSYS`).

---

## 3. Conformance & Verification Results

### 3.1 In-Kernel Programmatic Unit Tests (10/10 Passed)

```
[TEST SUITE 1] In-Kernel Milestone 13 Programmatic Unit Tests:
  [PASS] USER-TC1: Hardware Address Space Creation & Kernel Isolation
  [PASS] USER-TC2: Address Space Switching via CR3
  [PASS] USER-TC3: Ring 3 GDT & TSS Configuration
  [PASS] USER-TC4: System Call Dispatch via int 0x80
  [PASS] USER-TC5: Syscall Invalid Number Rejection
  [PASS] USER-TC6: Syscall User Pointer Validation
  [PASS] USER-TC7: Kernel Memory Protection from User Mode
  [PASS] USER-TC8: User Mode Execution and Safe Syscall Exit
  [PASS] USER-TC9: Address Space Destruction & Frame Reclamation
  [PASS] USER-TC10: Multi-Process Address Space Isolation
  [PASS] Overall Milestone 13 Test Suite Confirmation: ALL 10 TESTS PASSED.
```

### 3.2 Live User Mode Demonstration (5/5 Steps Passed)

```
[TEST SUITE 2] Live Milestone 13 User Mode & System Call Demonstration:
  [PASS] Step: Demo Banner
  [PASS] Step: Step 1: Process with CR3 (Created PID(0:g11) with private CR3=0x2de000)
  [PASS] Step: Step 2: App Loaded (Loaded 35-byte user application into User Space)
  [PASS] Step: Step 3: Ring 3 Transition (Transitioning CPU to Ring 3 via iretq...)
  [PASS] Step: User Console Output ([USER CONSOLE] Micro-OS User Process Running at Ring 3 via Hardware Isolation!)
  [PASS] Step: Step 4: Clean Exit (User Mode application completed cleanly via SYS_EXIT with code 0)
  [PASS] Step: Step 5: Reclamation (Cleaned up User Process and reclaimed all physical page frames)
  [PASS] Step: Idle Loop Banner
```

### 3.3 Preservation of Milestones 6–12 (100% Passed)

| Milestone | Subsystem | Unit Tests | Live Demo / Benchmarks | Status |
|---|---|---|---|---|
| **M6** | Task Management | 7 / 7 Passed | Task Lifecycle Demo | PASS |
| **M7** | Ready Queue | 12 / 12 Passed | 13 / 13 Auto-Sync Tests | PASS |
| **M8** | Cooperative Scheduler | 12 / 12 Passed | Cooperative Multi-Task Demo | PASS |
| **M9** | Preemptive Interrupts | 6 / 6 Passed | Live Preemption Demo (100 Hz PIT) | PASS |
| **M10** | Mutual Exclusion | 7 / 7 Passed | Race Condition vs Protected Demo | PASS |
| **M11** | Kernel Heap Allocator | 8 / 8 Passed | Dynamic Structures Demo | PASS |
| **M12** | Process & IPC | 10 / 10 Passed | H1–H5 Benchmarks & IPC Demo | PASS |
| **M13** | User Mode & Hardware Paging | 10 / 10 Passed | Ring 3 Syscall & Isolation Demo | **PASS** |

---

## 4. Visual VGA Screendump Artifact

- **Artifact Path:** `screenshots/vga_milestone13.png`
- **Resolution:** 720 x 400 pixels (standard VGA text mode)
- **Palette Breakdown:**
  - Black (Background): 88.31% (254,328 pixels)
  - Light Cyan (Banner Border): 1.10% (3,170 pixels)
  - White (Standard Log Text): 10.59% (30,502 pixels)

All tests passed with zero errors and zero regressions. Milestone 13 is fully implemented, verified, and complete.
