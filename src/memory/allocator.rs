//! Interrupt-safe kernel heap allocator and global allocator integration.
//!
//! Provides the global memory allocator for `rust-micro-os` using `linked_list_allocator::Heap`
//! wrapped in an interrupt-safe synchronization primitive that disables local CPU interrupts
//! during allocation and deallocation to prevent deadlocks with preemptive timer interrupts (M9).
//!
//! Implements a strict lifecycle state machine (Uninitialized -> Initializing -> Ready / Failed)
//! where `Ready` is published with `Release` memory ordering strictly after virtual page table
//! mapping and `Heap::init()` succeed. Re-initialization attempts are rejected to protect the live heap.
//!
//! Conforms to Milestone 11 requirements and CA-6 Section 8 / Track B specifications.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use linked_list_allocator::Heap;
use spin::Mutex;
use x86_64::instructions::interrupts;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size4KiB};
use x86_64::VirtAddr;

/// Virtual address where the kernel heap begins.
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// Size of the kernel heap: 256 KiB (64 pages of 4096 bytes).
pub const HEAP_SIZE: usize = 256 * 1024;

/// Lifecycle state of the kernel heap allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AllocatorState {
    /// Allocator has not been initialized. Allocation and deallocation fail safely.
    Uninitialized = 0,
    /// Allocator is currently undergoing virtual mapping and heap initialization.
    Initializing = 1,
    /// Allocator has completed page mapping and heap initialization; ready for service.
    Ready = 2,
    /// Allocator initialization failed (e.g. frame exhaustion or page table error).
    Failed = 3,
}

pub const STATUS_UNINITIALIZED: u8 = AllocatorState::Uninitialized as u8;
pub const STATUS_INITIALIZING: u8 = AllocatorState::Initializing as u8;
pub const STATUS_READY: u8 = AllocatorState::Ready as u8;
pub const STATUS_FAILED: u8 = AllocatorState::Failed as u8;

/// Error encountered during heap initialization.
#[derive(Debug)]
pub enum HeapInitError {
    /// Mapping error during virtual page table setup.
    MapError(MapToError<Size4KiB>),
    /// Heap has already been initialized (re-initialization rejected).
    AlreadyInitialized,
    /// Initialization is concurrently in progress.
    AlreadyInProgress,
}

impl PartialEq for HeapInitError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HeapInitError::AlreadyInitialized, HeapInitError::AlreadyInitialized) => true,
            (HeapInitError::AlreadyInProgress, HeapInitError::AlreadyInProgress) => true,
            _ => false,
        }
    }
}

impl Eq for HeapInitError {}

impl From<MapToError<Size4KiB>> for HeapInitError {
    fn from(err: MapToError<Size4KiB>) -> Self {
        HeapInitError::MapError(err)
    }
}

/// Telemetry metrics for the kernel allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorMetrics {
    pub acquire_count: u64,
    pub failure_count: u64,
    pub allocated_bytes: usize,
    pub deallocated_bytes: usize,
    pub used_bytes: usize,
    pub free_bytes: usize,
}

/// A wrapper around `spin::Mutex<A>` enforcing interrupt safety during lock acquisition
/// and managing the allocator's atomic lifecycle state machine.
pub struct Locked<A> {
    inner: Mutex<A>,
    acquire_count: AtomicU64,
    failure_count: AtomicU64,
    allocated_bytes: AtomicUsize,
    deallocated_bytes: AtomicUsize,
    status: AtomicU8,
}

impl<A> Locked<A> {
    /// Creates a new `Locked` container wrapping the given data in `Uninitialized` state.
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: Mutex::new(inner),
            acquire_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            allocated_bytes: AtomicUsize::new(0),
            deallocated_bytes: AtomicUsize::new(0),
            status: AtomicU8::new(STATUS_UNINITIALIZED),
        }
    }

    /// Acquires the inner mutex without disabling interrupts.
    pub fn lock(&self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }

    /// Returns the current lifecycle state of the allocator.
    pub fn state(&self) -> AllocatorState {
        match self.status.load(Ordering::Acquire) {
            STATUS_INITIALIZING => AllocatorState::Initializing,
            STATUS_READY => AllocatorState::Ready,
            STATUS_FAILED => AllocatorState::Failed,
            _ => AllocatorState::Uninitialized,
        }
    }

    /// Returns `true` if the allocator is published and ready for service.
    pub fn is_ready(&self) -> bool {
        self.status.load(Ordering::Acquire) == STATUS_READY
    }

    /// Directly sets the lifecycle status (used for localized testing of failure paths).
    pub fn set_status_for_testing(&self, new_status: u8) {
        self.status.store(new_status, Ordering::Release);
    }
}

unsafe impl GlobalAlloc for Locked<Heap> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Enforce interrupt safety: disable interrupts on local CPU while holding allocator lock
        interrupts::without_interrupts(|| {
            // Strictly require STATUS_READY with Acquire ordering before attempting allocation.
            // If the allocator is Uninitialized, Initializing, or Failed, fail safely.
            if self.status.load(Ordering::Acquire) != STATUS_READY {
                self.failure_count.fetch_add(1, Ordering::Relaxed);
                return core::ptr::null_mut();
            }

            self.acquire_count.fetch_add(1, Ordering::Relaxed);
            let mut heap = self.inner.lock();

            match heap.allocate_first_fit(layout) {
                Ok(allocation) => {
                    self.allocated_bytes.fetch_add(layout.size(), Ordering::Relaxed);
                    allocation.as_ptr()
                }
                Err(_) => {
                    self.failure_count.fetch_add(1, Ordering::Relaxed);
                    core::ptr::null_mut()
                }
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        // Enforce interrupt safety: disable interrupts on local CPU while freeing memory
        interrupts::without_interrupts(|| {
            if self.status.load(Ordering::Acquire) != STATUS_READY {
                return;
            }

            let addr = ptr as usize;
            // Bounds check: pointer must fall within the mapped heap range
            if addr < HEAP_START || addr.checked_add(layout.size()).map_or(true, |end| end > HEAP_START + HEAP_SIZE) {
                return;
            }

            self.deallocated_bytes.fetch_add(layout.size(), Ordering::Relaxed);
            let mut heap = self.inner.lock();
            heap.deallocate(NonNull::new_unchecked(ptr), layout);
        })
    }
}

/// The global kernel memory allocator.
#[global_allocator]
pub static ALLOCATOR: Locked<Heap> = Locked::new(Heap::empty());

/// Initializes the kernel heap by mapping virtual pages to physical frames
/// and providing the memory range to the `ALLOCATOR`.
///
/// Follows a strict lifecycle protocol:
/// 1. Transitions `Uninitialized` -> `Initializing` via atomic CAS.
/// 2. Rejects re-initialization if already `Ready` or `Initializing`.
/// 3. Maps heap virtual pages. On failure, transitions to `Failed` and returns error.
/// 4. Initializes the underlying `Heap`.
/// 5. Publishes `Ready` state with `Release` memory ordering strictly after initialization succeeds.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), HeapInitError> {
    // Atomic state transition: Uninitialized -> Initializing
    match ALLOCATOR.status.compare_exchange(
        STATUS_UNINITIALIZED,
        STATUS_INITIALIZING,
        Ordering::SeqCst,
        Ordering::SeqCst,
    ) {
        Ok(_) => {
            // Successfully claimed initialization ownership
        }
        Err(current) => {
            // Re-initialization attempt rejected to prevent resetting live heap
            return if current == STATUS_READY {
                Err(HeapInitError::AlreadyInitialized)
            } else if current == STATUS_INITIALIZING {
                Err(HeapInitError::AlreadyInProgress)
            } else {
                Err(HeapInitError::AlreadyInitialized)
            };
        }
    }

    let heap_start = VirtAddr::new(HEAP_START as u64);
    if let Err(e) = crate::memory::paging::map_heap_pages(mapper, frame_allocator, heap_start, HEAP_SIZE) {
        // Transition to Failed state upon mapping error
        ALLOCATOR.status.store(STATUS_FAILED, Ordering::Release);
        return Err(HeapInitError::MapError(e));
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    // Publish STATUS_READY with Release ordering strictly after Heap::init() has completed
    ALLOCATOR.status.store(STATUS_READY, Ordering::Release);

    Ok(())
}

/// Returns `true` if the kernel heap is published and ready for allocations.
pub fn is_heap_initialized() -> bool {
    ALLOCATOR.status.load(Ordering::Acquire) == STATUS_READY
}

/// Returns the current lifecycle state of the global allocator.
pub fn allocator_state() -> AllocatorState {
    ALLOCATOR.state()
}

/// Returns a snapshot of current allocator telemetry metrics.
pub fn get_metrics() -> AllocatorMetrics {
    let (used, free) = interrupts::without_interrupts(|| {
        let heap = ALLOCATOR.lock();
        (heap.used(), heap.free())
    });

    AllocatorMetrics {
        acquire_count: ALLOCATOR.acquire_count.load(Ordering::Relaxed),
        failure_count: ALLOCATOR.failure_count.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATOR.allocated_bytes.load(Ordering::Relaxed),
        deallocated_bytes: ALLOCATOR.deallocated_bytes.load(Ordering::Relaxed),
        used_bytes: used,
        free_bytes: free,
    }
}

/// Returns the total bytes currently allocated from the heap.
pub fn get_allocated_bytes() -> usize {
    interrupts::without_interrupts(|| ALLOCATOR.lock().used())
}

/// Returns the remaining free heap capacity in bytes.
pub fn get_free_bytes() -> usize {
    interrupts::without_interrupts(|| ALLOCATOR.lock().free())
}
