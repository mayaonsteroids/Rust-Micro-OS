//! Virtual memory paging and page table initialization.
//!
//! Provides the Level 4 Page Table interface using `x86_64::structures::paging::OffsetPageTable`
//! and handles virtual page mapping for the kernel heap.

use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, Size4KiB,
};
use x86_64::VirtAddr;

/// Initializes an `OffsetPageTable` pointing to the active Level 4 Page Table.
///
/// # Safety
/// The caller must ensure that the complete physical memory is mapped to virtual
/// memory at `physical_memory_offset`. Also, this function must only be called once
/// to avoid creating multiple aliasing `&mut PageTable` references.
pub unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let (level_4_table_frame, _) = x86_64::registers::control::Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    let level_4_table = &mut *page_table_ptr;
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Maps a contiguous virtual page range for the kernel heap to usable physical frames.
pub fn map_heap_pages(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    heap_start: VirtAddr,
    heap_size: usize,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_start + (heap_size as u64) - 1u64);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    Ok(())
}

/// Maps a physical memory range (such as a PCI MMIO BAR) into the kernel virtual address space
/// with uncacheable (NO_CACHE), writable, and present flags.
/// Returns the virtual address corresponding to `phys_addr`.
pub fn map_mmio_range(phys_addr: x86_64::PhysAddr, size: usize) -> VirtAddr {
    let phys_offset = VirtAddr::new(
        crate::memory::address_space::PHYSICAL_MEMORY_OFFSET.load(core::sync::atomic::Ordering::SeqCst),
    );
    let virt_addr = phys_offset + phys_addr.as_u64();

    let mut mapper = unsafe { init_offset_page_table(phys_offset) };
    use x86_64::structures::paging::Translate;

    let start_page: Page<Size4KiB> = Page::containing_address(virt_addr);
    let end_page: Page<Size4KiB> =
        Page::containing_address(virt_addr + (size as u64).saturating_sub(1));

    let mut fa_guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
    let frame_allocator = fa_guard.as_mut().expect("Frame allocator not initialized");

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

    for page in Page::range_inclusive(start_page, end_page) {
        if mapper.translate_addr(page.start_address()).is_none() {
            let page_phys = x86_64::PhysAddr::new(page.start_address().as_u64() - phys_offset.as_u64());
            let frame = x86_64::structures::paging::PhysFrame::containing_address(page_phys);
            unsafe {
                if let Ok(mapping) = mapper.map_to(page, frame, flags, frame_allocator) {
                    mapping.flush();
                }
            }
        }
    }

    virt_addr
}

