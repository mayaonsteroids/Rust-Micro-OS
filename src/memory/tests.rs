//! Comprehensive test suite and live demonstration for Kernel Memory Allocation.
//!
//! Conforms to Milestone 11 requirements and CA-6 Section 8 / Track B specifications:
//! - MEM-TC1: Basic Box Allocation & Dereference Verification.
//! - MEM-TC2: Dynamic Vector Growth & Reallocation (1,000 Elements).
//! - MEM-TC3: Multi-Alignment Verification (1, 4, 8, 16, 32, 64-byte Alignments).
//! - MEM-TC4: Memory Deallocation & Free-List Node Reuse.
//! - MEM-TC5: Safe Allocation Failure, Lifecycle State Machine & Re-Init Rejection.
//! - MEM-TC6: Interleaved High-Iteration Allocation Stress Test.
//! - MEM-TC7: Interrupt-Safe Allocator Locking Invariant.
//! - MEM-TC8: Educational Allocator Comparison (Bump vs Free-List with Double-Free Safety).

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use crate::memory::allocator::{
    self, AllocatorState, HeapInitError, ALLOCATOR, HEAP_SIZE, HEAP_START,
    STATUS_FAILED, STATUS_INITIALIZING,
};
use crate::memory::bump::BumpAllocator;
use crate::{error, info, warn};
use x86_64::structures::paging::mapper::{FlagUpdateError, MapToError, MapperFlush, TranslateError, UnmapError};
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};

/// Runs the complete Milestone 11 memory management test suite.
pub fn run_memory_unit_tests() -> bool {
    info!("========================================================");
    info!("      RUNNING MILESTONE 11: MEMORY ALLOCATOR TESTS      ");
    info!("========================================================");

    let mut all_passed = true;

    all_passed &= test_basic_box_allocation();
    all_passed &= test_dynamic_vector_growth();
    all_passed &= test_alignment_verification();
    all_passed &= test_deallocation_reuse();
    all_passed &= test_allocation_failure_safety();
    all_passed &= test_interleaved_stress();
    all_passed &= test_interrupt_safe_locking();
    all_passed &= test_bump_allocator_comparison();

    if all_passed {
        info!(">>> MILESTONE 11 MEMORY TESTS: ALL 8 TESTS PASSED <<<");
    } else {
        error!(">>> MILESTONE 11 MEMORY TESTS: VERIFICATION FAILED <<<");
    }

    all_passed
}

/// MEM-TC1: Basic Box Allocation & Dereference Verification.
fn test_basic_box_allocation() -> bool {
    const MAGIC: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let b = Box::new(MAGIC);

    let val = *b;
    let ptr_addr = &*b as *const u64 as usize;

    let in_bounds = ptr_addr >= HEAP_START && ptr_addr < (HEAP_START + HEAP_SIZE);
    let value_correct = val == MAGIC;

    drop(b);

    let pass = in_bounds && value_correct;
    if pass {
        info!(
            "  [PASS] MEM-TC1: Basic Box allocation verified (addr=0x{:x}, val=0x{:x}).",
            ptr_addr, val
        );
    } else {
        error!(
            "  [FAIL] MEM-TC1: Box allocation failed (addr=0x{:x}, in_bounds={}, val=0x{:x})!",
            ptr_addr, in_bounds, val
        );
    }
    pass
}

/// MEM-TC2: Dynamic Vector Growth & Reallocation (1,000 Elements).
fn test_dynamic_vector_growth() -> bool {
    const COUNT: u32 = 1_000;
    let mut vec = Vec::new();

    for i in 0..COUNT {
        vec.push(i);
    }

    let len_ok = vec.len() == COUNT as usize;
    let cap_ok = vec.capacity() >= COUNT as usize;

    let mut sum: u64 = 0;
    for &x in vec.iter() {
        sum += x as u64;
    }
    let expected_sum: u64 = ((COUNT as u64 - 1) * COUNT as u64) / 2; // 499,500
    let sum_ok = sum == expected_sum;

    drop(vec);

    let pass = len_ok && cap_ok && sum_ok;
    if pass {
        info!(
            "  [PASS] MEM-TC2: Dynamic vector growth verified ({} elements, sum={}).",
            COUNT, sum
        );
    } else {
        error!(
            "  [FAIL] MEM-TC2: Dynamic vector growth failed (len_ok={}, cap_ok={}, sum_ok={})!",
            len_ok, cap_ok, sum_ok
        );
    }
    pass
}

/// MEM-TC3: Multi-Alignment Verification (1, 4, 8, 16, 32, 64-byte Alignments).
fn test_alignment_verification() -> bool {
    let alignments = [1, 4, 8, 16, 32, 64];
    let mut all_aligned = true;

    for &align in &alignments {
        let layout = match Layout::from_size_align(64, align) {
            Ok(l) => l,
            Err(_) => {
                all_aligned = false;
                break;
            }
        };

        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        if ptr.is_null() {
            all_aligned = false;
            error!("  [FAIL] MEM-TC3: Allocation returned null for alignment {}!", align);
            break;
        }

        let addr = ptr as usize;
        let is_aligned = (addr % align) == 0;
        if !is_aligned {
            all_aligned = false;
            error!(
                "  [FAIL] MEM-TC3: Address 0x{:x} is not aligned to {} bytes!",
                addr, align
            );
        }

        unsafe {
            ALLOCATOR.dealloc(ptr, layout);
        }

        if !is_aligned {
            break;
        }
    }

    if all_aligned {
        info!("  [PASS] MEM-TC3: Multi-alignment (1, 4, 8, 16, 32, 64 bytes) verified.");
    }
    all_aligned
}

/// MEM-TC4: Memory Deallocation & Free-List Node Reuse.
fn test_deallocation_reuse() -> bool {
    let initial_used = allocator::get_allocated_bytes();

    // Allocate 50 boxes
    let mut boxes = Vec::new();
    for i in 0..50 {
        boxes.push(Box::new(i * 10));
    }

    let mid_used = allocator::get_allocated_bytes();
    let memory_increased = mid_used > initial_used;

    // Drop all boxes
    drop(boxes);
    let after_drop_used = allocator::get_allocated_bytes();
    let memory_reclaimed = after_drop_used < mid_used;

    // Re-allocate 50 boxes of the exact same size; should reuse the freed blocks
    let mut boxes2 = Vec::new();
    for i in 0..50 {
        boxes2.push(Box::new(i * 20));
    }
    let realloc_used = allocator::get_allocated_bytes();

    // The re-allocated heap footprint should be virtually identical to mid_used
    let reuse_confirmed = realloc_used <= mid_used + 64;

    drop(boxes2);

    let pass = memory_increased && memory_reclaimed && reuse_confirmed;
    if pass {
        info!(
            "  [PASS] MEM-TC4: Deallocation and free-list memory reuse verified (used: {} -> {} -> {} -> {}).",
            initial_used, mid_used, after_drop_used, realloc_used
        );
    } else {
        error!(
            "  [FAIL] MEM-TC4: Memory reuse failed (initial={}, mid={}, after_drop={}, realloc={})!",
            initial_used, mid_used, after_drop_used, realloc_used
        );
    }
    pass
}

/// Dummy Page Table Mapper for verifying that re-initialization of an already Ready heap
/// is rejected immediately without ever accessing or invoking page table operations.
struct DummyReinitMapper;

impl Mapper<Size4KiB> for DummyReinitMapper {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        _page: Page<Size4KiB>,
        _frame: PhysFrame<Size4KiB>,
        _flags: PageTableFlags,
        _parent_table_flags: PageTableFlags,
        _frame_allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        panic!("DummyReinitMapper::map_to_with_table_flags invoked on re-initialization!");
    }

    fn unmap(
        &mut self,
        _page: Page<Size4KiB>,
    ) -> Result<(PhysFrame<Size4KiB>, MapperFlush<Size4KiB>), UnmapError> {
        panic!("DummyReinitMapper::unmap invoked!");
    }

    unsafe fn update_flags(
        &mut self,
        _page: Page<Size4KiB>,
        _flags: PageTableFlags,
    ) -> Result<MapperFlush<Size4KiB>, FlagUpdateError> {
        panic!("DummyReinitMapper::update_flags invoked!");
    }

    unsafe fn set_flags_p4_entry(
        &mut self,
        _page: Page<Size4KiB>,
        _flags: PageTableFlags,
    ) -> Result<x86_64::structures::paging::mapper::MapperFlushAll, FlagUpdateError> {
        panic!("DummyReinitMapper::set_flags_p4_entry invoked!");
    }

    unsafe fn set_flags_p3_entry(
        &mut self,
        _page: Page<Size4KiB>,
        _flags: PageTableFlags,
    ) -> Result<x86_64::structures::paging::mapper::MapperFlushAll, FlagUpdateError> {
        panic!("DummyReinitMapper::set_flags_p3_entry invoked!");
    }

    unsafe fn set_flags_p2_entry(
        &mut self,
        _page: Page<Size4KiB>,
        _flags: PageTableFlags,
    ) -> Result<x86_64::structures::paging::mapper::MapperFlushAll, FlagUpdateError> {
        panic!("DummyReinitMapper::set_flags_p2_entry invoked!");
    }

    fn translate_page(&self, _page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError> {
        panic!("DummyReinitMapper::translate_page invoked!");
    }
}

/// Dummy Frame Allocator for verifying re-initialization rejection.
struct DummyReinitFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for DummyReinitFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        panic!("DummyReinitFrameAllocator::allocate_frame invoked on re-initialization!");
    }
}

/// MEM-TC5: Safe Allocation Failure, Lifecycle State Machine & Re-Init Rejection.
fn test_allocation_failure_safety() -> bool {
    let small_layout = Layout::from_size_align(32, 8).unwrap();

    // 1. Lifecycle verification: Uninitialized state fails safely
    let test_allocator: allocator::Locked<linked_list_allocator::Heap> =
        allocator::Locked::new(linked_list_allocator::Heap::empty());
    assert_eq!(test_allocator.state(), AllocatorState::Uninitialized);
    let uninit_ptr = unsafe { test_allocator.alloc(small_layout) };
    let uninit_failed_safely = uninit_ptr.is_null();

    // 2. Lifecycle verification: Initializing state fails safely
    test_allocator.set_status_for_testing(STATUS_INITIALIZING);
    assert_eq!(test_allocator.state(), AllocatorState::Initializing);
    let init_ptr = unsafe { test_allocator.alloc(small_layout) };
    let init_failed_safely = init_ptr.is_null();

    // 3. Lifecycle verification: Failed state fails safely
    test_allocator.set_status_for_testing(STATUS_FAILED);
    assert_eq!(test_allocator.state(), AllocatorState::Failed);
    let failed_ptr = unsafe { test_allocator.alloc(small_layout) };
    let failed_state_safely = failed_ptr.is_null();

    // 4. Verification of Global Allocator publication in Ready state
    let global_ready = ALLOCATOR.state() == AllocatorState::Ready && allocator::is_heap_initialized();

    // 5. Verification of Re-initialization Rejection
    // An attempt to re-initialize the active global heap must return Err(AlreadyInitialized)
    // and must never invoke the mapper or frame allocator or corrupt the live heap.
    let mut dummy_mapper = DummyReinitMapper;
    let mut dummy_frames = DummyReinitFrameAllocator;
    let reinit_result = allocator::init_heap(&mut dummy_mapper, &mut dummy_frames);
    let reinit_rejected = reinit_result == Err(HeapInitError::AlreadyInitialized);

    // 6. Request 10 MiB from the global allocator, whereas heap is 256 KiB (OOM test)
    let huge_layout = match Layout::from_size_align(10 * 1024 * 1024, 8) {
        Ok(l) => l,
        Err(_) => return false,
    };

    let initial_failures = allocator::get_metrics().failure_count;
    let ptr = unsafe { ALLOCATOR.alloc(huge_layout) };
    let oom_failed_safely = ptr.is_null();
    let after_failures = allocator::get_metrics().failure_count;
    let failure_recorded = after_failures > initial_failures;

    // 7. Verify heap integrity: subsequent normal allocation must still succeed!
    let recovery_box = Box::new(4242u32);
    let recovery_ok = *recovery_box == 4242;
    drop(recovery_box);

    let pass = uninit_failed_safely
        && init_failed_safely
        && failed_state_safely
        && global_ready
        && reinit_rejected
        && oom_failed_safely
        && failure_recorded
        && recovery_ok;

    if pass {
        info!("  [PASS] MEM-TC5: Safe allocation failure, lifecycle states & re-init rejection verified.");
    } else {
        error!(
            "  [FAIL] MEM-TC5: Failure handling failed (uninit={}, init={}, failed={}, ready={}, reinit={}, oom={}, recorded={}, recovered={})!",
            uninit_failed_safely, init_failed_safely, failed_state_safely, global_ready, reinit_rejected, oom_failed_safely, failure_recorded, recovery_ok
        );
    }
    pass
}

/// MEM-TC6: Interleaved High-Iteration Allocation Stress Test.
fn test_interleaved_stress() -> bool {
    const ITERATIONS: usize = 100;
    let mut ptrs: [*mut u8; ITERATIONS] = [core::ptr::null_mut(); ITERATIONS];
    let mut layouts: [Layout; ITERATIONS] = [Layout::new::<u8>(); ITERATIONS];
    let mut sizes: [usize; ITERATIONS] = [0; ITERATIONS];

    let mut pass = true;

    // Interleave allocations of varying sizes
    for i in 0..ITERATIONS {
        let size = (i % 8 + 1) * 16; // 16 to 128 bytes
        let align = 8;
        let layout = match Layout::from_size_align(size, align) {
            Ok(l) => l,
            Err(_) => return false,
        };

        layouts[i] = layout;
        sizes[i] = size;
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        if ptr.is_null() {
            error!("  [FAIL] MEM-TC6: Allocation failed at iteration {}", i);
            pass = false;
            break;
        }

        // Write a test pattern into allocated memory
        unsafe {
            core::ptr::write_bytes(ptr, (i & 0xFF) as u8, size);
        }
        ptrs[i] = ptr;
    }

    // Verify memory contents across all 100 blocks to prove zero corruption and zero overlap
    for i in 0..ITERATIONS {
        if !ptrs[i].is_null() {
            let expected_byte = (i & 0xFF) as u8;
            for offset in 0..sizes[i] {
                let actual_byte = unsafe { core::ptr::read(ptrs[i].add(offset)) };
                if actual_byte != expected_byte {
                    error!(
                        "  [FAIL] MEM-TC6: Data corruption at block {} offset {}: expected 0x{:x}, got 0x{:x}",
                        i, offset, expected_byte, actual_byte
                    );
                    pass = false;
                    break;
                }
            }
        }
    }

    // Deallocate in reverse order
    for i in (0..ITERATIONS).rev() {
        if !ptrs[i].is_null() {
            unsafe {
                ALLOCATOR.dealloc(ptrs[i], layouts[i]);
            }
        }
    }

    if pass {
        info!("  [PASS] MEM-TC6: Interleaved stress test ({} allocations, zero overlap, all patterns intact) verified.", ITERATIONS);
    }
    pass
}

/// MEM-TC7: Interrupt-Safe Allocator Locking Invariant.
fn test_interrupt_safe_locking() -> bool {
    let original_if = x86_64::instructions::interrupts::are_enabled();

    // Part A: Verify state preservation when interrupts are initially DISABLED
    x86_64::instructions::interrupts::disable();
    let b1 = Box::new(11111111u64);
    assert_eq!(*b1, 11111111);
    let disabled_after_alloc = !x86_64::instructions::interrupts::are_enabled();
    drop(b1);
    let disabled_after_drop = !x86_64::instructions::interrupts::are_enabled();

    // Part B: Verify state preservation when interrupts are initially ENABLED
    x86_64::instructions::interrupts::enable();
    let b2 = Box::new(22222222u64);
    assert_eq!(*b2, 22222222);
    let enabled_after_alloc = x86_64::instructions::interrupts::are_enabled();
    drop(b2);
    let enabled_after_drop = x86_64::instructions::interrupts::are_enabled();

    // Restore original interrupt flag state upon test exit
    if !original_if {
        x86_64::instructions::interrupts::disable();
    }

    let pass = disabled_after_alloc && disabled_after_drop && enabled_after_alloc && enabled_after_drop;
    if pass {
        info!("  [PASS] MEM-TC7: Interrupt-safe allocator locking invariant (IF=0 and IF=1 states) verified.");
    } else {
        error!(
            "  [FAIL] MEM-TC7: Interrupt state preservation failed (dis_alloc={}, dis_drop={}, en_alloc={}, en_drop={})!",
            disabled_after_alloc, disabled_after_drop, enabled_after_alloc, enabled_after_drop
        );
    }
    pass
}

/// MEM-TC8: Educational Allocator Comparison (Bump vs Free-List with Double-Free Safety).
fn test_bump_allocator_comparison() -> bool {
    // Static buffer for testing the Bump Allocator
    static mut BUMP_BUFFER: [u8; 1024] = [0; 1024];

    let mut bump = BumpAllocator::new();
    let base_addr = core::ptr::addr_of!(BUMP_BUFFER) as usize;
    unsafe {
        bump.init(base_addr, 1024);
    }

    assert_eq!(bump.used(), 0);
    assert_eq!(bump.remaining(), 1024);
    assert_eq!(bump.active_allocations(), 0);

    // 1. Allocate 3 blocks: A (64B), B (128B), C (256B)
    let l_a = Layout::from_size_align(64, 8).unwrap();
    let l_b = Layout::from_size_align(128, 16).unwrap();
    let l_c = Layout::from_size_align(256, 32).unwrap();

    let p_a = bump.alloc(l_a).unwrap();
    let p_b = bump.alloc(l_b).unwrap();
    let p_c = bump.alloc(l_c).unwrap();

    let valid_ptrs = !p_a.is_null() && !p_b.is_null() && !p_c.is_null();
    let allocations_after_3 = bump.active_allocations() == 3;
    let used_after_3 = bump.used();

    // 2. Test Invalid Pointer, Size, and Alignment Rejection:
    // a. Null pointer
    let null_rejected = !bump.dealloc(core::ptr::null_mut(), l_a);
    // b. Misaligned / unallocated internal offset
    let offset_rejected = !bump.dealloc(unsafe { p_a.add(3) }, l_a);
    // c. Alien pointer outside heap buffer
    let alien_rejected = !bump.dealloc(0xDEAD_BEEF as *mut u8, l_a);
    // d. Incorrect layout size
    let wrong_size_layout = Layout::from_size_align(65, 8).unwrap();
    let wrong_size_rejected = !bump.dealloc(p_a, wrong_size_layout);
    // e. Incorrect layout alignment (valid pointer p_b and correct size 128, but align=32 instead of 16)
    let wrong_align_layout_b = Layout::from_size_align(128, 32).unwrap();
    let wrong_align_b_rejected = !bump.dealloc(p_b, wrong_align_layout_b);
    let b_remains_live = bump.is_allocated(p_b);

    // Verify invalid operations did NOT alter allocations or bump pointer
    let invalid_rejected = null_rejected
        && offset_rejected
        && alien_rejected
        && wrong_size_rejected
        && wrong_align_b_rejected
        && b_remains_live
        && bump.active_allocations() == 3
        && bump.used() == used_after_3;

    // 3. Test Freeing in Out-of-Order Sequence: Free B (middle allocation) first using exact matching layout
    let free_b_ok = bump.dealloc(p_b, l_b);
    let after_free_b_allocs = bump.active_allocations() == 2;
    let after_free_b_no_reset = bump.used() == used_after_3; // next did not reset because A and C are live!

    // 4. Test Duplicate Free Rejection on Block B:
    let free_b_again = bump.dealloc(p_b, l_b);
    let dup_b_rejected = !free_b_again;
    let dup_b_allocs_intact = bump.active_allocations() == 2; // did NOT decrement!
    let dup_b_used_intact = bump.used() == used_after_3; // did NOT reset!

    // 5. Deallocate A, then attempt duplicate free on A
    let free_a_ok = bump.dealloc(p_a, l_a);
    let after_free_a_allocs = bump.active_allocations() == 1;
    let after_free_a_no_reset = bump.used() == used_after_3; // C is still live!

    let free_a_again = bump.dealloc(p_a, l_a);
    let dup_a_rejected = !free_a_again;
    let dup_a_allocs_intact = bump.active_allocations() == 1; // did NOT decrement!
    let dup_a_used_intact = bump.used() == used_after_3; // did NOT reset while C is live!

    // 5b. Attempt deallocating C with correct pointer and size (256B) but wrong alignment (16 instead of 32)
    let wrong_align_c = Layout::from_size_align(256, 16).unwrap();
    let wrong_align_c_rejected = !bump.dealloc(p_c, wrong_align_c);
    let c_remains_live = bump.is_allocated(p_c) && bump.active_allocations() == 1 && bump.used() == used_after_3;

    // 6. Reset ONLY when all valid allocations are released: Deallocate C (final block) using exact matching layout
    let free_c_ok = bump.dealloc(p_c, l_c);
    let reset_on_empty = bump.active_allocations() == 0 && bump.used() == 0 && bump.remaining() == 1024;

    // 7. Verify buffer reusability from base after full reset
    let p_new = bump.alloc(l_a).unwrap();
    let reused_base = (p_new as usize) == base_addr;
    bump.dealloc(p_new, l_a);

    let pass = valid_ptrs
        && allocations_after_3
        && invalid_rejected
        && free_b_ok
        && after_free_b_allocs
        && after_free_b_no_reset
        && dup_b_rejected
        && dup_b_allocs_intact
        && dup_b_used_intact
        && free_a_ok
        && after_free_a_allocs
        && after_free_a_no_reset
        && dup_a_rejected
        && dup_a_allocs_intact
        && dup_a_used_intact
        && wrong_align_c_rejected
        && c_remains_live
        && free_c_ok
        && reset_on_empty
        && reused_base;

    if pass {
        info!("  [PASS] MEM-TC8: Educational allocator comparison (BumpAllocator tracking, double-free safety & reset) verified.");
    } else {
        error!(
            "  [FAIL] MEM-TC8: Bump allocator verification failed (valid={}, invalid_rej={}, b_ok={}, dup_b_rej={}, a_ok={}, dup_a_rej={}, c_align_rej={}, c_live={}, reset={}, reuse={})!",
            valid_ptrs, invalid_rejected, free_b_ok, dup_b_rejected, free_a_ok, dup_a_rejected, wrong_align_c_rejected, c_remains_live, reset_on_empty, reused_base
        );
    }
    pass
}

// -----------------------------------------------------------------------------
// LIVE HEAP ALLOCATION & DYNAMIC DATA STRUCTURES DEMONSTRATION
// -----------------------------------------------------------------------------

/// Demonstrates dynamic data structures, real-time memory tracking, and heap recycling.
pub fn run_memory_demo() -> bool {
    info!("--- BEGIN DEMO: Kernel Heap Allocation & Dynamic Data Structures ---");

    let metrics_before = allocator::get_metrics();
    info!(
        "[Heap Baseline] Used: {} bytes | Free: {} bytes | Total Ops: {}",
        metrics_before.used_bytes, metrics_before.free_bytes, metrics_before.acquire_count
    );

    // Step 1: Allocate a dynamic array of task records
    info!("Demo Step 1: Allocating dynamic task buffer using Vec<DynamicTaskDescriptor>...");
    #[allow(dead_code)]
    struct DynamicTaskDescriptor {
        id: u32,
        name: &'static str,
        priority: u8,
        stack_size: usize,
    }

    let mut task_list: Vec<DynamicTaskDescriptor> = Vec::new();
    task_list.push(DynamicTaskDescriptor { id: 101, name: "FileSystemWorker", priority: 2, stack_size: 4096 });
    task_list.push(DynamicTaskDescriptor { id: 102, name: "NetworkDispatch", priority: 1, stack_size: 8192 });
    task_list.push(DynamicTaskDescriptor { id: 103, name: "CryptoHasher", priority: 3, stack_size: 4096 });

    let metrics_step1 = allocator::get_metrics();
    info!(
        "  [Step 1 Result] Allocated {} tasks in vector. Heap Used: {} bytes (+{} bytes).",
        task_list.len(),
        metrics_step1.used_bytes,
        metrics_step1.used_bytes.saturating_sub(metrics_before.used_bytes)
    );

    // Step 2: Allocate boxed structures
    info!("Demo Step 2: Allocating recursive tree node using Box<T>...");
    struct TreeNode {
        key: u32,
        left: Option<Box<TreeNode>>,
        right: Option<Box<TreeNode>>,
    }

    let tree = Box::new(TreeNode {
        key: 50,
        left: Some(Box::new(TreeNode { key: 25, left: None, right: None })),
        right: Some(Box::new(TreeNode { key: 75, left: None, right: None })),
    });

    let metrics_step2 = allocator::get_metrics();
    info!(
        "  [Step 2 Result] Allocated 3-node binary tree. Root: {}, Left: {}, Right: {}.",
        tree.key,
        tree.left.as_ref().unwrap().key,
        tree.right.as_ref().unwrap().key
    );
    info!(
        "  [Step 2 Metrics] Heap Used: {} bytes | Free: {} bytes.",
        metrics_step2.used_bytes, metrics_step2.free_bytes
    );

    // Step 3: Deallocate tree and tasks, verify memory recycling
    info!("Demo Step 3: Deallocating tree and task buffer, verifying free-list memory reclamation...");
    drop(tree);
    drop(task_list);

    let metrics_step3 = allocator::get_metrics();
    info!(
        "  [Step 3 Result] Heap Used: {} bytes | Reclaimed: {} bytes.",
        metrics_step3.used_bytes,
        metrics_step2.used_bytes.saturating_sub(metrics_step3.used_bytes)
    );

    let demo_passed = metrics_step3.used_bytes <= metrics_before.used_bytes + 256;
    if demo_passed {
        info!("Demo Success: Kernel heap allocation, dynamic structures, and memory recycling verified.");
    } else {
        warn!("Demo Note: Heap used after deallocation is {}", metrics_step3.used_bytes);
    }

    demo_passed
}
