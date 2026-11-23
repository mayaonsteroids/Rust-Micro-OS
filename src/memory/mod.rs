//! Memory management subsystem for the `rust-micro-os` kernel.
//!
//! Encompasses:
//! - Physical page frame allocation via `frame_allocator::BootInfoFrameAllocator`.
//! - Virtual memory page mapping via `paging::OffsetPageTable`.
//! - Interrupt-safe heap allocation via `allocator::ALLOCATOR`.
//! - Educational bump allocation via `bump::BumpAllocator`.
//! - Comprehensive verification tests via `tests`.

pub mod address_space;
pub mod allocator;
pub mod bump;
pub mod frame_allocator;
pub mod paging;
pub mod tests;

use bootloader::BootInfo;
use x86_64::VirtAddr;
use crate::{info, error};

pub use address_space::{
    get_memory_stats, AddressSpace, GlobalFrameAllocator, MemoryStats, USER_CODE_VIRT_ADDR,
    USER_DATA_VIRT_ADDR, USER_SPACE_MAX, USER_SPACE_MIN, USER_STACK_TOP, USER_STACK_VIRT_ADDR,
};
pub use allocator::{HEAP_SIZE, HEAP_START};
pub use frame_allocator::BootInfoFrameAllocator;

/// Initializes the complete memory management subsystem:
/// 1. Initializes the Level 4 Page Table interface (`OffsetPageTable`).
/// 2. Initializes the physical frame allocator (`BootInfoFrameAllocator`).
/// 3. Maps virtual pages for the kernel heap and initializes the global allocator.
/// 4. Initializes the persistent `GlobalFrameAllocator` and records kernel CR3 for hardware isolation.
pub fn init(boot_info: &'static BootInfo) -> Result<(), &'static str> {
    info!("========================================================");
    info!("         INITIALIZING KERNEL MEMORY SUBSYSTEM           ");
    info!("========================================================");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    // 1. Initialize OffsetPageTable
    let mut mapper = unsafe { paging::init_offset_page_table(phys_mem_offset) };

    // 2. Initialize Frame Allocator from BootInfo memory map
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    let usable_bytes = frame_allocator.usable_memory_bytes();
    let total_frames = frame_allocator.total_usable_frames();
    info!(
        "[MEMORY] Physical RAM: {} usable frames detected (~{} KiB / {} MiB available).",
        total_frames,
        usable_bytes / 1024,
        usable_bytes / (1024 * 1024)
    );
    info!("[MEMORY] Physical memory virtual mapping offset: 0x{:016x}", boot_info.physical_memory_offset);

    // 3. Map heap pages and initialize global heap allocator
    match allocator::init_heap(&mut mapper, &mut frame_allocator) {
        Ok(()) => {
            info!(
                "[MEMORY] Kernel Heap successfully mapped: [0x{:x} .. 0x{:x}] ({} KiB).",
                HEAP_START,
                HEAP_START + HEAP_SIZE,
                HEAP_SIZE / 1024
            );
            info!(
                "[MEMORY] Physical frames allocated for heap: {} (Remaining: {}).",
                frame_allocator.allocated_frames(),
                total_frames.saturating_sub(frame_allocator.allocated_frames())
            );

            // 4. Initialize persistent GlobalFrameAllocator for hardware paging isolation
            address_space::init_global_frame_allocator(frame_allocator, phys_mem_offset);
            info!("[MEMORY] Global frame allocator initialized for hardware paging isolation.");

            Ok(())
        }
        Err(e) => {
            error!("[MEMORY] Failed to initialize kernel heap: {:?}", e);
            Err("Heap initialization failed")
        }
    }
}
