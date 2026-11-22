//! Educational Bump Allocator with Active Allocation Tracking.
//!
//! Demonstrates the linear pointer bumping allocation strategy:
//! - Allocates linearly from a contiguous memory buffer with alignment rounding.
//! - Tracks active allocations in a bounded metadata table to enforce double-free safety,
//!   reject invalid or duplicate pointers, and ensure the bump pointer resets to the base
//!   *only* when every live allocation has been genuinely released.
//! - Requires exact matching of pointer address, layout size, and layout alignment on deallocation.
//!
//! Part of CA-6 Layer 8 and Track B (Memory Engineering) specialization.

use core::alloc::Layout;

/// Maximum number of concurrent active allocations tracked by the educational bump allocator.
pub const MAX_BUMP_ALLOCATIONS: usize = 64;

/// Metadata record for an active allocation within `BumpAllocator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationRecord {
    pub ptr: usize,
    pub size: usize,
    pub align: usize,
}

/// Calculates the smallest integer multiple of `align` that is greater than or equal to `addr`.
///
/// # Panics
/// Panics if `align` is not a power of two.
pub fn align_up(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two(), "align must be a power of two");
    (addr + align - 1) & !(align - 1)
}

/// A bump allocator with robust allocation tracking, alignment validation, and double-free rejection.
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
    records: [Option<AllocationRecord>; MAX_BUMP_ALLOCATIONS],
}

impl BumpAllocator {
    /// Creates an empty bump allocator.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
            records: [None; MAX_BUMP_ALLOCATIONS],
        }
    }

    /// Initializes the bump allocator with a given memory region.
    ///
    /// # Safety
    /// The caller must ensure that the given memory range is unused and valid.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
        self.allocations = 0;
        self.records = [None; MAX_BUMP_ALLOCATIONS];
    }

    /// Allocates a block of memory with the given layout.
    ///
    /// Enforces alignment, bounds checks against `heap_end`, overflow detection via
    /// `checked_add`, and capacity bounds on the active allocation table.
    /// Records pointer address, size, and alignment in the metadata table.
    pub fn alloc(&mut self, layout: Layout) -> Result<*mut u8, ()> {
        if self.allocations >= MAX_BUMP_ALLOCATIONS {
            return Err(()); // Active allocation tracking table capacity exhausted
        }

        let alloc_start = align_up(self.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return Err(()), // Integer overflow
        };

        if alloc_end > self.heap_end {
            return Err(()); // Out of memory
        }

        // Find an empty record slot
        let slot = match self.records.iter_mut().find(|r| r.is_none()) {
            Some(s) => s,
            None => return Err(()),
        };

        *slot = Some(AllocationRecord {
            ptr: alloc_start,
            size: layout.size(),
            align: layout.align(),
        });

        self.next = alloc_end;
        self.allocations += 1;
        Ok(alloc_start as *mut u8)
    }

    /// Deallocates an allocation.
    ///
    /// A linear bump allocator cannot reclaim arbitrary individual blocks out-of-order.
    /// To guarantee memory safety:
    /// 1. `ptr` is validated: non-null and within `[heap_start .. next)`.
    /// 2. `records` is searched for an exact active allocation matching:
    ///    - pointer address (`record.ptr == addr`)
    ///    - layout size (`record.size == layout.size()`)
    ///    - layout alignment (`record.align == layout.align()`)
    /// 3. If found, the record is cleared and `allocations` is decremented.
    /// 4. If NOT found (e.g. mismatched alignment, mismatched size, duplicate free,
    ///    already freed, invalid pointer, or alien pointer), the deallocation is rejected
    ///    returning `false` without modifying `allocations`, `records`, or `next`.
    /// 5. The bump pointer `next` resets to `heap_start` ONLY when `allocations == 0` (all genuine
    ///    live allocations have been released).
    ///
    /// Returns `true` if deallocation succeeded, or `false` if rejected.
    pub fn dealloc(&mut self, ptr: *mut u8, layout: Layout) -> bool {
        if ptr.is_null() {
            return false;
        }

        let addr = ptr as usize;
        // Range check: pointer must fall within the allocated buffer range
        if addr < self.heap_start || addr >= self.next {
            return false;
        }

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
                // Clear the active record
                *entry = None;
                self.allocations = self.allocations.saturating_sub(1);

                // Reset bump pointer ONLY when all active allocations have been released
                if self.allocations == 0 {
                    self.next = self.heap_start;
                }
                true
            }
            None => {
                // Mismatched alignment/size, duplicate free, already freed, or unregistered pointer:
                // Reject without modifying self.allocations, self.records, or self.next
                false
            }
        }
    }

    /// Returns `true` if the given pointer is currently recorded as an active allocation.
    pub fn is_allocated(&self, ptr: *mut u8) -> bool {
        if ptr.is_null() {
            return false;
        }
        let addr = ptr as usize;
        self.records.iter().any(|r| {
            if let Some(record) = r {
                record.ptr == addr
            } else {
                false
            }
        })
    }

    /// Returns the number of bytes currently consumed by the bump pointer.
    pub fn used(&self) -> usize {
        self.next.saturating_sub(self.heap_start)
    }

    /// Returns the number of remaining free bytes in the buffer.
    pub fn remaining(&self) -> usize {
        self.heap_end.saturating_sub(self.next)
    }

    /// Returns the active number of allocations.
    pub fn active_allocations(&self) -> usize {
        self.allocations
    }

    /// Resets the allocator unconditionally to the beginning of the heap.
    pub fn reset(&mut self) {
        self.next = self.heap_start;
        self.allocations = 0;
        self.records = [None; MAX_BUMP_ALLOCATIONS];
    }
}
