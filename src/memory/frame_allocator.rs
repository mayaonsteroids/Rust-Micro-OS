//! BootInfo Frame Allocator for physical memory management.
//!
//! Inspects the BIOS/UEFI memory map provided by the bootloader and dispenses
//! free physical 4096-byte frames (`PhysFrame<Size4KiB>`) that do not collide
//! with kernel code, kernel stacks, page tables, or reserved hardware areas.

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

/// A physical frame allocator that uses the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Creates a `BootInfoFrameAllocator` from the passed memory map.
    ///
    /// # Safety
    /// The caller must guarantee that the passed `memory_map` is valid and that all
    /// frames marked `Usable` are not in use anywhere else in the kernel.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over all usable physical frames from the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        let addr_ranges = usable_regions.map(|r| r.range.start_frame_number..r.range.end_frame_number);
        let frame_numbers = addr_ranges.flat_map(|r| r);
        frame_numbers.map(|f| PhysFrame::containing_address(PhysAddr::new(f * 4096)))
    }

    /// Returns the total number of usable physical frames detected in the system.
    pub fn total_usable_frames(&self) -> usize {
        self.usable_frames().count()
    }

    /// Returns the number of physical frames allocated so far by this allocator.
    pub fn allocated_frames(&self) -> usize {
        self.next
    }

    /// Returns the total usable physical RAM in bytes.
    pub fn usable_memory_bytes(&self) -> u64 {
        let mut total = 0u64;
        for region in self.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                let frames = region.range.end_frame_number - region.range.start_frame_number;
                total += frames * 4096;
            }
        }
        total
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
