# Milestone 11: Kernel Memory Allocator — Implementation & Verification Report

**Course:** Operating Systems (CA-6 Flagship Engineering Project)  
**Milestone:** 11 of 17 — Memory Allocator  
**Status:** **Implemented, Tested in QEMU, Audited, and Verified (Exit Code 0)**  
**Artifacts:**  
- Source Directory: [`src/memory/`](file:///e:/MICRO%20OS/src/memory/)
- Test Script: [`scripts/test_milestone11.py`](file:///e:/MICRO%20OS/scripts/test_milestone11.py)
- VGA Screenshot: [`screenshots/vga_milestone11.png`](file:///e:/MICRO%20OS/screenshots/vga_milestone11.png) / [`vga_milestone11.png`](file:///C:/Users/roshi/.gemini/antigravity/brain/7557e3cf-60c1-4710-bc94-8a4900f862d6/vga_milestone11.png)

---

## 1. Executive Summary

Milestone 11 establishes a bare-metal **Kernel Memory Management & Dynamic Heap Allocation Subsystem** in the `rust-micro-os` kernel, fulfilling the core requirements of CA-6 Layer 8 and the advanced specialization criteria of Track B (Memory Engineering).

Following the rigorous source-code audit, two critical safety issues were resolved:
1. **Issue 1 — `BumpAllocator` Double-Free Safety (`src/memory/bump.rs`)**: Replaced the naive address range check with an active allocation record metadata tracking table (`[Option<AllocationRecord>; MAX_BUMP_ALLOCATIONS]`). Duplicate frees, alien pointers, and invalid offsets are rejected without modifying the allocation counter or bump pointer. The bump pointer resets to base *only* when all genuine live allocations are released.
2. **Issue 2 — Global Allocator Initialization Publication (`src/memory/allocator.rs`)**: Implemented an atomic lifecycle state machine (`Uninitialized (0) -> Initializing (1) -> Ready (2) / Failed (3)`). The `Ready` state is published with `Release` memory ordering strictly *after* virtual page table mapping and `Heap::init()` succeed. Allocation attempts prior to `Ready` fail safely. Repeated calls to `init_heap()` are rejected with `Err(HeapInitError::AlreadyInitialized)` to prevent resetting or corrupting a live heap.

### Key Deliverables Completed:
1. **Physical Frame Allocator (`BootInfoFrameAllocator`)**: An iterator-driven physical memory frame allocator parsing the BIOS/UEFI `bootloader::bootinfo::MemoryMap`. Filters and dispenses usable 4096-byte physical frames (`PhysFrame<Size4KiB>`) while guaranteeing zero collision with kernel executable code, task stacks, page tables, or reserved hardware areas.
2. **Virtual Memory Paging (`OffsetPageTable`)**: Maps the kernel heap's virtual address range (`0x4444_4444_0000 .. 0x4444_4444_0000 + 256 KiB`) to freshly allocated physical frames using the active Level 4 page table and the bootloader's physical memory virtual offset.
3. **Interrupt-Safe Global Allocator (`Locked<linked_list_allocator::Heap>`)**: Integrates `linked_list_allocator::Heap` as the system `#[global_allocator]`. Encapsulates allocation and deallocation operations within `x86_64::instructions::interrupts::without_interrupts`, eliminating deadlocks with Milestone 9's 100 Hz timer interrupts and Milestone 10's synchronization locks.
4. **Allocator Telemetry & Tracking**: Real-time atomic metrics tracking total successful allocations (`acquire_count`), allocation failures (`failure_count`), allocated bytes, freed bytes, and current heap usage.
5. **Educational `BumpAllocator` (Track B Extension)**: A standalone linear bump allocator demonstrating monotonic pointer advancement, power-of-two alignment rounding, active allocation metadata tracking, double-free rejection, and total-reset reclamation.
6. **Native Rust `alloc` Crate Integration**: Enabled `extern crate alloc;`, unlocking standard dynamic collections (`Box<T>`, `Vec<T>`, `String`) across the kernel for future IPC and task subsystems.
7. **In-Kernel Test Suite (MEM-TC1 to MEM-TC8)**: Comprehensive unit tests validating basic boxed allocation, dynamic vector expansion, multi-alignment invariants (1, 4, 8, 16, 32, 64 bytes), deallocation and free-list memory reuse, safe out-of-memory handling and lifecycle publication, interleaved high-iteration stress testing, interrupt-safe locking under both `IF=0` and `IF=1`, and educational allocator comparison with double-free safety.
8. **Live Heap Allocation Demonstration**: Demonstrated dynamic vector creation, recursive binary tree construction, and memory recycling in QEMU.
9. **Full Milestone 1–10 Preservation**: All previous milestone test suites (M6 task lifecycle, M7 ready queue, M8 cooperative scheduler, M9 timer preemption, M10 synchronization primitives) remain 100% operational and regression-free.

---

## 2. Architecture and OS Mechanism Design

### 2.1 The Need for Dynamic Kernel Memory
Prior to Milestone 11, `rust-micro-os` relied exclusively on statically allocated arrays (`MAX_TASKS = 32`, fixed 4 KiB stacks, static circular buffers). While static allocation provides deterministic latency in early boot, it imposes strict constraints:
- Variable-length kernel data structures (e.g., dynamically sized task queues, message buffers, inter-process communication channels) cannot scale at runtime.
- Recursive or dynamic algorithms (e.g., process trees, memory graphs) cannot be represented cleanly without a heap.
- Bare-metal Rust standard library types (`Box`, `Vec`, `String`) require an active implementation of the `core::alloc::GlobalAlloc` trait.

### 2.2 Memory Management Subsystem Architecture

```text
+-------------------------------------------------------------------------+
|                  Rust Standard Collections (Box, Vec)                   |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|         #[global_allocator] Locked<linked_list_allocator::Heap>         |
|      - State Machine: Uninitialized -> Initializing -> Ready / Failed   |
|      - Interrupt Safety: interrupts::without_interrupts(|| ...)         |
|      - Free-List Node Management & First-Fit Block Search               |
|      - Telemetry: acquire_count, failure_count, allocated/freed bytes   |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                    Virtual Address Space for Heap                       |
|           [0x4444_4444_0000 .. 0x4444_4444_0000 + 256 KiB]             |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|            Paging & Mapping Layer (OffsetPageTable)                     |
|      - Translates Virtual Pages -> Physical Frames (4 KiB)              |
|      - Writes Level 4, 3, 2, 1 Page Tables with PRESENT | WRITABLE      |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|            Physical Frame Allocator (BootInfoFrameAllocator)            |
|      - Reads Bootloader MemoryMap (Filters MemoryRegionType::Usable)    |
|      - Protects Kernel Code, Task Stacks, Page Tables, BIOS/Hardware    |
+-------------------------------------------------------------------------+
```

### 2.3 Interrupt Safety and Deadlock Defense
With timer-driven preemptive scheduling running at 100 Hz from Milestone 9, an allocation in task context could be interrupted mid-operation. If an Interrupt Service Routine (ISR) or preemptive scheduler routine attempted to allocate memory or access allocator telemetry while the lock was held:
- On a single-core system, spinning with `IF = 0` would permanently deadlock the CPU.
- **Defense Mechanism:** Our `Locked<Heap>` implementation wraps every invocation of `alloc()` and `dealloc()` in `x86_64::instructions::interrupts::without_interrupts`. This disables local CPU interrupts (`cli`) before taking the lock and restores the initial `RFLAGS.IF` state upon exit. Consequently:
  1. A timer interrupt can never preempt a task while the allocator lock is held.
  2. ISRs can safely call telemetry or allocation routines without risking lock contention with interrupted code.

---

## 3. Detailed Audit Resolution & Technical Corrections

### 3.1 Issue 1 Resolution: `BumpAllocator` Double-Free Safety (`src/memory/bump.rs`)

#### Root Cause Analysis
Previously, `BumpAllocator::dealloc()` attempted an address range check:
```rust
if addr >= self.heap_start && addr < self.next {
    self.allocations = self.allocations.saturating_sub(1);
    if self.allocations == 0 {
        self.next = self.heap_start;
    }
}
```
**Defect:** This range check only verified that the pointer fell somewhere between the base of the heap and the bump pointer. It could not differentiate a genuine, currently live allocation from an already-freed allocation or an internal offset. If a caller allocated blocks A and B, deallocated A, and then called `dealloc(A)` a second time, the allocation counter would decrement from 1 to 0, triggering `self.next = self.heap_start` while block B was still actively in use!

#### Architectural Solution
We redesigned `BumpAllocator` to maintain a bounded table of active allocation records:
```rust
pub const MAX_BUMP_ALLOCATIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationRecord {
    pub ptr: usize,
    pub size: usize,
    pub align: usize,
}

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
    records: [Option<AllocationRecord>; MAX_BUMP_ALLOCATIONS],
}
```

1. **Active Allocation Registration (`alloc`)**:
   When `alloc(layout)` succeeds, it stores `Some(AllocationRecord { ptr: alloc_start, size: layout.size(), align: layout.align() })` in an unoccupied slot and increments `self.allocations`.
2. **Exact Identity Matching (Address, Size, Alignment) & Rejection (`dealloc`)**:
   ```rust
   pub fn dealloc(&mut self, ptr: *mut u8, layout: Layout) -> bool {
       if ptr.is_null() { return false; }
       let addr = ptr as usize;
       if addr < self.heap_start || addr >= self.next { return false; }

       // Search for an active allocation record matching pointer, size, and alignment exactly
       let slot = self.records.iter_mut().find(|r| {
           if let Some(record) = r {
               record.ptr == addr && record.size == layout.size() && record.align == layout.align()
           } else {
               false
           }
       });

       match slot {
           Some(entry) => {
               *entry = None;
               self.allocations = self.allocations.saturating_sub(1);
               if self.allocations == 0 {
                   self.next = self.heap_start;
               }
               true
           }
           None => {
               // Mismatched alignment/size, duplicate free, or alien pointer rejected!
               // Leaves self.allocations, self.records, and self.next completely unmodified.
               false
           }
       }
   }
   ```
3. **Verification Invariants Proved in `MEM-TC8`**:
   - **Invalid Pointer & Mismatched Layout Rejection**: Null pointers, offset internal pointers (`p_a + 3`), alien pointers (`0xDEADBEEF`), incorrect layout sizes, and **mismatched layout alignments** (valid pointer and size, but incorrect alignment) are rejected returning `false`, without modifying `allocations` or `next`.
   - **Live Persistence After Mismatched Alignment**: Proved that when deallocating block $B$ or $C$ with incorrect alignment, the allocation remains active and live (`is_allocated(ptr) == true`), and can subsequently be correctly deallocated using its exact matching layout.
   - **Out-of-Order Deallocation**: Freeing block B first decrements `allocations` from 3 to 2, but leaves `next` untouched because A and C are still active.
   - **Duplicate Free Rejection**: Calling `dealloc(B)` a second time returns `false` and leaves `allocations = 2` intact. Calling `dealloc(A)` twice decrements `allocations` to 1 and rejects the second call, leaving `allocations = 1` intact. The bump pointer *never* resets while block C is live.
   - **Reset Only When Empty**: Deallocating block C decrements `allocations` to 0, which triggers `self.next = self.heap_start`. A subsequent allocation reuses the buffer from the base address (`p_new == p_a`).

---

### 3.2 Issue 2 Resolution: Global Allocator Initialization Publication (`src/memory/allocator.rs`)

#### Root Cause Analysis
Previously, `init_heap()` ran:
```rust
if ALLOCATOR.initialized.compare_exchange(false, true, ...).is_err() {
    return Ok(());
}
crate::memory::paging::map_heap_pages(...)?;
ALLOCATOR.lock().init(...);
```
**Defect:** The `initialized` boolean was set to `true` *before* `map_heap_pages()` and `Heap::init()` completed. If an allocation occurred or if an interrupt routine inspected `is_heap_initialized()`, it would observe `true` on an unmapped or uninitialized heap. Furthermore, if mapping failed, `initialized` was set to `false`, leaving a transient window of inconsistency.

#### Architectural Solution
We implemented a 4-state lifecycle state machine with atomic memory ordering:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AllocatorState {
    Uninitialized = 0,
    Initializing = 1,
    Ready = 2,
    Failed = 3,
}
```

1. **Exclusive Initialization Claim**:
   `init_heap()` executes an atomic CAS from `STATUS_UNINITIALIZED (0)` to `STATUS_INITIALIZING (1)`:
   ```rust
   match ALLOCATOR.status.compare_exchange(
       STATUS_UNINITIALIZED,
       STATUS_INITIALIZING,
       Ordering::SeqCst,
       Ordering::SeqCst,
   ) {
       Ok(_) => { /* Proceed with mapping and init */ }
       Err(current) => {
           return if current == STATUS_READY {
               Err(HeapInitError::AlreadyInitialized)
           } else if current == STATUS_INITIALIZING {
               Err(HeapInitError::AlreadyInProgress)
           } else {
               Err(HeapInitError::AlreadyInitialized)
           };
       }
   }
   ```
2. **Safe Failure on Uninitialized / Initializing States**:
   In `GlobalAlloc::alloc()`:
   ```rust
   if self.status.load(Ordering::Acquire) != STATUS_READY {
       self.failure_count.fetch_add(1, Ordering::Relaxed);
       return core::ptr::null_mut();
   }
   ```
   Any allocation attempt while the heap is `Uninitialized`, `Initializing`, or `Failed` immediately and safely returns `null_mut()`.
3. **Publication With Release Ordering**:
   Only after `crate::memory::paging::map_heap_pages()` succeeds and `ALLOCATOR.lock().init()` finishes does `init_heap()` publish the ready state:
   ```rust
   ALLOCATOR.status.store(STATUS_READY, Ordering::Release);
   ```
   This ensures that all page table writes and heap metadata initialization are visible to any CPU core or thread that observes `STATUS_READY` via `Acquire` ordering.
4. **Re-Initialization Rejection**:
   Subsequent calls to `init_heap()` on an active heap observe `current == STATUS_READY` and return `Err(HeapInitError::AlreadyInitialized)`. The live heap is never re-mapped, never re-initialized, and never corrupted.
5. **Verification Invariants Proved in `MEM-TC5`**:
   - `Uninitialized` allocator returns `null_mut()`.
   - `Initializing` allocator returns `null_mut()`.
   - `Failed` allocator returns `null_mut()`.
   - Global allocator publishes `AllocatorState::Ready`.
   - Re-initialization attempt with dummy mapper/allocator returns `Err(HeapInitError::AlreadyInitialized)` without calling page table routines.
   - Out-of-memory allocation (10 MiB) returns `null_mut()`, logs telemetry failure, and allows immediate heap recovery.

---

## 4. In-Kernel Unit Test Suite Breakdown (MEM-TC1 to MEM-TC8)

| Test ID | Test Name | Invariant Verified | Execution Mode | Result |
|---|---|---|---|---|
| **MEM-TC1** | Basic Box Allocation | Allocates `Box::new(MAGIC)`, verifies pointer lies inside `[0x444444440000 .. 0x444444480000]`, verifies value, and safely drops. | Sequential (Boot Context) | **PASS** |
| **MEM-TC2** | Dynamic Vector Growth | Creates `Vec<u32>`, pushes 1,000 elements across multiple capacity reallocations, and verifies mathematical checksum ($499,500$). | Sequential (Boot Context) | **PASS** |
| **MEM-TC3** | Multi-Alignment Invariant | Allocates memory with alignments of 1, 4, 8, 16, 32, and 64 bytes; asserts that `addr % align == 0` for all layouts. | Sequential (Boot Context) | **PASS** |
| **MEM-TC4** | Deallocation & Reuse | Allocates 50 boxes, frees them, and re-allocates 50 boxes; confirms that free-list nodes are recycled without heap growth. | Sequential (Boot Context) | **PASS** |
| **MEM-TC5** | Lifecycle & OOM Safety | Verifies safe failure in `Uninitialized`, `Initializing`, and `Failed` states; proves `Ready` publication; verifies re-initialization rejection; handles 10 MiB OOM safely with full recovery. | Sequential (Boot Context) | **PASS** |
| **MEM-TC6** | Interleaved Stress Test | Performs 100 interleaved allocations of variable sizes (16 to 128 bytes); writes byte patterns; **verifies all byte patterns intact across all 100 blocks (zero overlap)**; deallocates in reverse order. | Sequential (Boot Context) | **PASS** |
| **MEM-TC7** | Interrupt-Safe Locking | Verifies that heap operations preserve CPU `RFLAGS.IF` state under both initial conditions: **Case A (initially disabled, IF=0)** and **Case B (initially enabled, IF=1)**. | Sequential (Hardware IRQ State) | **PASS** |
| **MEM-TC8** | Educational Comparison | Demonstrates `BumpAllocator` tracking, invalid pointer rejection, out-of-order freeing, double-free rejection, and reset only when all active allocations are released. | Sequential (Boot Context) | **PASS** |

---

## 5. Live Heap Allocation Demonstration Results

### Serial Console Output:
```text
[INFO] --- BEGIN DEMO: Kernel Heap Allocation & Dynamic Data Structures ---
[INFO] [Heap Baseline] Used: 0 bytes | Free: 262144 bytes | Total Ops: 230
[INFO] Demo Step 1: Allocating dynamic task buffer using Vec<DynamicTaskDescriptor>...
[INFO]   [Step 1 Result] Allocated 3 tasks in vector. Heap Used: 128 bytes (+128 bytes).
[INFO] Demo Step 2: Allocating recursive tree node using Box<T>...
[INFO]   [Step 2 Result] Allocated 3-node binary tree. Root: 50, Left: 25, Right: 75.
[INFO]   [Step 2 Metrics] Heap Used: 200 bytes | Free: 261944 bytes.
[INFO] Demo Step 3: Deallocating tree and task buffer, verifying free-list memory reclamation...
[INFO]   [Step 3 Result] Heap Used: 0 bytes | Reclaimed: 200 bytes.
[INFO] Demo Success: Kernel heap allocation, dynamic structures, and memory recycling verified.
[INFO] Live heap allocation & dynamic data structures demonstration verified successfully.
[INFO] All Milestone 11 tasks and demonstrations completed. Entering idle loop.
```

---

## 6. Programmatic Verification Evidence

### 6.1 Verification Commands Executed
```powershell
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && cargo check"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && cargo bootimage"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 -m py_compile scripts/test_milestone11.py"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 scripts/test_milestone11.py"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 scripts/test_milestone10.py"
wsl -u roshi bash -lc "cd '/mnt/e/MICRO OS' && python3 scripts/test_milestone9.py"
```

### 6.2 Test Suite Execution Summary
```text
=================================================================
        RUST MICRO-OS: MILESTONE 11 VERIFICATION AUDIT           
=================================================================
[PRESERVATION CHECK] Milestone 6 Task Tests: ALL 7 TESTS PASSED.
[PRESERVATION CHECK] Milestone 7 Ready Queue Tests: ALL 12 TESTS PASSED.
[PRESERVATION CHECK] Milestone 7 Auto-Sync Regression Tests: ALL 13 TESTS PASSED.
[PRESERVATION CHECK] Milestone 8 Scheduler Tests: ALL 12 TESTS PASSED.
[PRESERVATION CHECK] Milestone 8 Cooperative Multi-Task Demo: PASSED.
[PRESERVATION CHECK] Milestone 9 Interrupt Tests: ALL 6 TESTS PASSED.
[PRESERVATION CHECK] Milestone 9 Live Preemptive Multi-Task Demo: PASSED.
[PRESERVATION CHECK] Milestone 10 Synchronization Tests: ALL 7 TESTS PASSED.
[PRESERVATION CHECK] Milestone 10 Live Synchronization Demo: PASSED.

[SUBSYSTEM INITIALIZATION] Kernel Memory & Heap Subsystem:
  [PASS] Memory Banner
  [PASS] Physical RAM Detection
  [PASS] Virtual Mapping Offset
  [PASS] Heap Page Mapping
  [PASS] Physical Frame Allocation

[TEST SUITE 1] In-Kernel Memory Allocator Programmatic Unit Tests:
  [PASS] MEM-TC1: Basic Box allocation
  [PASS] MEM-TC2: Dynamic vector growth
  [PASS] MEM-TC3: Multi-alignment verification
  [PASS] MEM-TC4: Deallocation & memory reuse
  [PASS] MEM-TC5: Safe allocation failure handling
  [PASS] MEM-TC6: Interleaved stress test
  [PASS] MEM-TC7: Interrupt-safe allocator locking
  [PASS] MEM-TC8: Educational allocator comparison
  [PASS] Overall Memory Test Suite Confirmation: ALL 8 TESTS PASSED.

[TEST SUITE 2] Live Heap Allocation & Dynamic Data Structures Demonstration:
  [PASS] Step: Demo Banner
  [PASS] Step: Heap Baseline
  [PASS] Step: Step 1 Banner
  [PASS] Step: Step 1 Result
  [PASS] Step: Step 2 Banner
  [PASS] Step: Step 2 Result
  [PASS] Step: Step 2 Metrics
  [PASS] Step: Step 3 Banner
  [PASS] Step: Step 3 Result
  [PASS] Step: Demo Success
  [PASS] Step: Idle Loop Banner

-----------------------------------------------------------------
>>> AUTOMATED ASSERTIONS RESULT: ALL PASS (0 ERRORS) <<<
-----------------------------------------------------------------
```

---

## 7. College Viva & Examination Defense Guide

### Q1: What is the difference between physical frame allocation and virtual heap allocation?
**Answer:** A physical frame allocator manages raw physical RAM frames (typically 4096 bytes each) provided by hardware and the BIOS memory map. A heap allocator operates on a contiguous range of *virtual* memory addresses. The paging subsystem (via page tables) bridges the two by mapping virtual pages to physical frames, allowing the heap allocator to hand out arbitrary byte-sized allocations without worrying about physical memory fragmentation or discontiguity.

### Q2: Why is `without_interrupts` necessary when acquiring the heap allocator lock?
**Answer:** In a preemptive kernel with hardware timer interrupts (like our 100 Hz PIT from Milestone 9), a task could be preempted while holding the allocator lock. If an interrupt handler (or another task on a single-core CPU that was interrupted) attempts to allocate memory or inspect heap metrics, it would spin waiting for the lock. Because interrupts are disabled in the ISR and the single core cannot run the lock holder, the system would permanently deadlock. Disabling interrupts during allocation ensures atomic, uninterrupted heap operations.

### Q3: Why is a simple address range check insufficient to protect a Bump Allocator from double-free corruption?
**Answer:** A range check `addr >= heap_start && addr < next` only confirms that the pointer lies somewhere inside the allocated buffer window. It cannot verify whether that specific pointer is currently active or has already been freed. If a caller deallocates pointer A twice, a naive counter decrement would decrement the active allocation count to zero and reset the bump pointer to `heap_start`, even while block B is still live. Tracking active allocation metadata in an allocation record table guarantees that duplicate frees are rejected and the bump pointer resets only when every live allocation is genuinely released.

### Q4: How does atomic lifecycle state publication prevent uninitialized heap access?
**Answer:** By maintaining an atomic state enum (`Uninitialized -> Initializing -> Ready / Failed`), any allocation attempt prior to successful page mapping and `Heap::init()` loads the state with `Acquire` ordering, observes that `state != Ready`, and safely returns `null_mut()`. `Ready` is stored with `Release` memory ordering strictly after initialization succeeds, ensuring all page table mappings and internal heap structures are synchronized and visible before any allocation can proceed.

---

## 8. Known Limitations & Future Work

1. **Bounded Educational Metadata Table**: `BumpAllocator` uses a bounded static array (`MAX_BUMP_ALLOCATIONS = 64`) to track active allocation metadata without requiring dynamic memory. If an application attempts more than 64 concurrent bump allocations before a reset, `alloc()` safely returns `Err(())`.
2. **Coarse-Grained Linear Bump Reclamation**: While `BumpAllocator` now strictly validates individual pointers, sizes, alignments, and duplicate frees, it is by design an educational linear allocator: individual holes are not reclaimed until all active allocations are released (`allocations == 0`), resetting the pointer to the base.
3. **Fixed Global Kernel Heap Size**: The global kernel heap is statically sized at 256 KiB (64 pages of 4096 bytes). In a full production OS, the paging subsystem would implement on-demand page table expansion when the heap approaches capacity.
4. **First-Fit Allocation Search Complexity**: `linked_list_allocator` traverses its free-list using first-fit search ($O(N)$ in the number of free blocks). A production operating system may implement segregated-fit or buddy allocators ($O(1)$) to minimize fragmentation.

---

## 9. Final Audit Verdict

- **Issue 1 (`BumpAllocator` exact address, size, and alignment matching with double-free safety) fully implemented and verified in `MEM-TC8`.**
- **Issue 2 (Global allocator lifecycle state machine and publication) fully implemented and verified in `MEM-TC5`.**
- **All 8 in-kernel memory unit tests pass with zero errors.**
- **Milestones 1–10 continue to pass with zero regressions.**
- **Milestone 12 has NOT been started.**

### **FINAL VERDICT: READY FOR FORMAL APPROVAL**
