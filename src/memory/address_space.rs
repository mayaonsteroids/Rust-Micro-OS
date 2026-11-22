//! Hardware Memory Isolation: Per-process Level 4 Page Tables, CR3 address space switching,
//! and physical frame ownership tracking.
//!
//! Enforces hardware-level protection between User Mode (Ring 3) and Kernel Mode (Ring 0)
//! conforming to Milestone 13 requirements.

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};
use crate::memory::frame_allocator::BootInfoFrameAllocator;

/// Maximum number of physical frames that can be retained in the free-frame recycle list.
pub const FREE_LIST_CAPACITY: usize = 128;

/// Maximum number of physical frames owned by a single user address space.
pub const MAX_OWNED_FRAMES: usize = 16;

/// Canonical User Mode virtual address layout (L4 entry index 1 = 512 GiB boundary):
pub const USER_CODE_VIRT_ADDR: u64 = 0x0000_0080_0000_0000;
pub const USER_STACK_VIRT_ADDR: u64 = 0x0000_0080_0001_0000;
pub const USER_STACK_SIZE: u64 = 4096;
pub const USER_STACK_TOP: u64 = USER_STACK_VIRT_ADDR + USER_STACK_SIZE;
pub const USER_DATA_VIRT_ADDR: u64 = 0x0000_0080_0002_0000;
pub const USER_SPACE_MIN: u64 = 0x0000_0080_0000_0000;
pub const USER_SPACE_MAX: u64 = 0x0000_0080_0003_0000;

/// Global physical memory virtual mapping offset provided by the bootloader.
pub static PHYSICAL_MEMORY_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Physical frame of the primary kernel Level 4 Page Table.
pub static KERNEL_L4_FRAME: Mutex<Option<PhysFrame>> = Mutex::new(None);

/// Optional test hook to simulate physical allocation failure during address space construction.
pub static SIMULATE_ALLOC_FAILURE_AFTER: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(usize::MAX);

/// Global physical frame allocator with explicit free-list recycling.
pub struct GlobalFrameAllocator {
    pub boot_allocator: BootInfoFrameAllocator,
    pub freed_frames: [Option<PhysFrame>; FREE_LIST_CAPACITY],
    pub freed_count: usize,
    pub allocated_count: usize,
    pub deallocated_count: usize,
}

impl GlobalFrameAllocator {
    pub fn new(boot_allocator: BootInfoFrameAllocator) -> Self {
        Self {
            boot_allocator,
            freed_frames: [None; FREE_LIST_CAPACITY],
            freed_count: 0,
            allocated_count: 0,
            deallocated_count: 0,
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // First check the free list for recycled frames
        if self.freed_count > 0 {
            for slot in self.freed_frames.iter_mut() {
                if let Some(frame) = slot.take() {
                    self.freed_count -= 1;
                    self.allocated_count += 1;
                    return Some(frame);
                }
            }
        }
        // Otherwise allocate a fresh frame from the boot memory map
        let frame = self.boot_allocator.allocate_frame();
        if frame.is_some() {
            self.allocated_count += 1;
        }
        frame
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame) -> Result<(), &'static str> {
        if self.freed_count >= FREE_LIST_CAPACITY {
            return Err("Frame free-list full");
        }
        for slot in self.freed_frames.iter_mut() {
            if slot.is_none() {
                *slot = Some(frame);
                self.freed_count += 1;
                self.deallocated_count += 1;
                return Ok(());
            }
        }
        Err("No vacant slot in free-list")
    }
}

unsafe impl FrameAllocator<Size4KiB> for GlobalFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_frame()
    }
}

/// Global synchronized frame allocator instance.
pub static FRAME_ALLOCATOR: Mutex<Option<GlobalFrameAllocator>> = Mutex::new(None);

/// Initializes the global frame allocator and records the kernel L4 frame and physical memory offset.
pub fn init_global_frame_allocator(
    boot_allocator: BootInfoFrameAllocator,
    phys_offset: VirtAddr,
) {
    PHYSICAL_MEMORY_OFFSET.store(phys_offset.as_u64(), Ordering::SeqCst);
    let (l4_frame, _) = Cr3::read();
    *KERNEL_L4_FRAME.lock() = Some(l4_frame);
    *FRAME_ALLOCATOR.lock() = Some(GlobalFrameAllocator::new(boot_allocator));
}

/// Converts a physical address to a virtual address using the bootloader's complete physical memory mapping.
#[inline(always)]
pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    let offset = PHYSICAL_MEMORY_OFFSET.load(Ordering::Relaxed);
    VirtAddr::new(phys.as_u64() + offset)
}

/// Allocates a single physical frame from the global allocator.
pub fn allocate_frame() -> Option<PhysFrame> {
    FRAME_ALLOCATOR.lock().as_mut()?.allocate_frame()
}

/// Deallocates a physical frame back to the global allocator free-list.
pub fn deallocate_frame(frame: PhysFrame) -> Result<(), &'static str> {
    let mut guard = FRAME_ALLOCATOR.lock();
    let fa = guard.as_mut().ok_or("FRAME_ALLOCATOR not initialized")?;
    fa.deallocate_frame(frame)
}

/// Represents an isolated hardware address space (per-process Level 4 Page Table hierarchy).
pub struct AddressSpace {
    /// Physical frame of this address space's Level 4 Page Table (written to CR3).
    pub cr3: PhysFrame,
    /// Physical frames owned and managed by this address space (reclaimed upon destruction).
    pub owned_frames: [Option<PhysFrame>; MAX_OWNED_FRAMES],
    pub owned_count: usize,
    /// Physical frame backing the user code page (`0x0000_0080_0000_0000`).
    pub user_code_frame: PhysFrame,
    /// Physical frame backing the user stack page (`0x0000_0080_0001_0000`).
    pub user_stack_frame: PhysFrame,
    /// Physical frame backing the user data page (`0x0000_0080_0002_0000`).
    pub user_data_frame: PhysFrame,
}

impl AddressSpace {
    /// Constructs a new hardware address space:
    /// 1. Allocates an L4 page table frame.
    /// 2. Clears the L4 table.
    /// 3. Copies all kernel mappings from the kernel's active L4 table (stripping `USER_ACCESSIBLE`).
    /// 4. Allocates L3, L2, L1 page tables for L4 entry 1 (User Space) with `USER_ACCESSIBLE`.
    /// 5. Allocates user code, user stack, and user data physical frames.
    pub fn new() -> Result<Self, &'static str> {
        let mut owned_frames = [None; MAX_OWNED_FRAMES];
        let mut owned_count = 0;

        // Helper closure to allocate and record ownership with simulated failure check
        let alloc_owned = |frames: &mut [Option<PhysFrame>; MAX_OWNED_FRAMES], count: &mut usize| -> Result<PhysFrame, &'static str> {
            let fail_after = SIMULATE_ALLOC_FAILURE_AFTER.load(Ordering::SeqCst);
            if *count >= fail_after {
                return Err("Simulated allocation failure during construction");
            }
            let frame = allocate_frame().ok_or("Physical frame allocation failed")?;
            if *count < MAX_OWNED_FRAMES {
                frames[*count] = Some(frame);
                *count += 1;
                Ok(frame)
            } else {
                let _ = deallocate_frame(frame);
                Err("AddressSpace owned_frames capacity exceeded")
            }
        };

        let construct = |frames: &mut [Option<PhysFrame>; MAX_OWNED_FRAMES], count: &mut usize| -> Result<Self, &'static str> {
            // 1. Allocate L4 table frame
            let l4_frame = alloc_owned(frames, count)?;
            let l4_virt = phys_to_virt(l4_frame.start_address());
            let l4_table = unsafe { &mut *(l4_virt.as_mut_ptr() as *mut PageTable) };
            l4_table.zero();

            // 2. Copy kernel mappings from the primary kernel L4 table
            let kernel_l4_frame = KERNEL_L4_FRAME.lock().ok_or("Kernel L4 not initialized")?;
            let kernel_l4_virt = phys_to_virt(kernel_l4_frame.start_address());
            let kernel_l4 = unsafe { &*(kernel_l4_virt.as_ptr() as *const PageTable) };

            for i in 0..512 {
                // Reserve L4 entry 1 exclusively for user-mode address space
                if i != 1 && !kernel_l4[i].is_unused() {
                    let addr = kernel_l4[i].addr();
                    // Strictly guarantee that all kernel mappings are supervisor-only (NOT user accessible)
                    let flags = kernel_l4[i].flags() & !PageTableFlags::USER_ACCESSIBLE;
                    l4_table[i].set_addr(addr, flags);
                }
            }

            // 3. Allocate page tables for User Space (L4 index 1: 0x0000_0080_0000_0000 .. 0x0000_00ff_ffff_ffff)
            let l3_frame = alloc_owned(frames, count)?;
            let l3_virt = phys_to_virt(l3_frame.start_address());
            let l3_table = unsafe { &mut *(l3_virt.as_mut_ptr() as *mut PageTable) };
            l3_table.zero();

            let l2_frame = alloc_owned(frames, count)?;
            let l2_virt = phys_to_virt(l2_frame.start_address());
            let l2_table = unsafe { &mut *(l2_virt.as_mut_ptr() as *mut PageTable) };
            l2_table.zero();

            let l1_frame = alloc_owned(frames, count)?;
            let l1_virt = phys_to_virt(l1_frame.start_address());
            let l1_table = unsafe { &mut *(l1_virt.as_mut_ptr() as *mut PageTable) };
            l1_table.zero();

            // Wire page table hierarchy with USER_ACCESSIBLE | PRESENT | WRITABLE
            let user_table_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
            l4_table[1].set_addr(l3_frame.start_address(), user_table_flags);
            l3_table[0].set_addr(l2_frame.start_address(), user_table_flags);
            l2_table[0].set_addr(l1_frame.start_address(), user_table_flags);

            // 4. Allocate user code page (L1 index 0 = 0x0000_0080_0000_0000)
            let user_code_frame = alloc_owned(frames, count)?;
            let code_virt = phys_to_virt(user_code_frame.start_address());
            unsafe {
                core::ptr::write_bytes(code_virt.as_mut_ptr::<u8>(), 0, 4096);
            }
            l1_table[0].set_addr(user_code_frame.start_address(), user_table_flags);

            // 5. Allocate user stack page (L1 index 16 = 0x0000_0080_0001_0000)
            let user_stack_frame = alloc_owned(frames, count)?;
            let stack_virt = phys_to_virt(user_stack_frame.start_address());
            unsafe {
                core::ptr::write_bytes(stack_virt.as_mut_ptr::<u8>(), 0, 4096);
            }
            l1_table[16].set_addr(user_stack_frame.start_address(), user_table_flags);

            // 6. Allocate user data page (L1 index 32 = 0x0000_0080_0002_0000)
            let user_data_frame = alloc_owned(frames, count)?;
            let data_virt = phys_to_virt(user_data_frame.start_address());
            unsafe {
                core::ptr::write_bytes(data_virt.as_mut_ptr::<u8>(), 0, 4096);
            }
            l1_table[32].set_addr(user_data_frame.start_address(), user_table_flags);

            Ok(Self {
                cr3: l4_frame,
                owned_frames: *frames,
                owned_count: *count,
                user_code_frame,
                user_stack_frame,
                user_data_frame,
            })
        };

        match construct(&mut owned_frames, &mut owned_count) {
            Ok(aspace) => Ok(aspace),
            Err(e) => {
                // Deallocate any partially allocated frames on construction failure to prevent leaks
                for slot in owned_frames.iter_mut() {
                    if let Some(frame) = slot.take() {
                        let _ = deallocate_frame(frame);
                    }
                }
                Err(e)
            }
        }
    }

    /// Loads binary executable bytes into this address space's user code page.
    pub fn load_code(&mut self, code: &[u8]) -> Result<(), &'static str> {
        if code.len() > 4096 {
            return Err("Code exceeds 4096-byte page size");
        }
        let virt = phys_to_virt(self.user_code_frame.start_address());
        unsafe {
            let dest = virt.as_mut_ptr::<u8>();
            core::ptr::copy_nonoverlapping(code.as_ptr(), dest, code.len());
        }
        Ok(())
    }

    /// Activates this address space on the current CPU by updating CR3.
    pub fn switch_to(&self) {
        unsafe {
            let (curr_cr3, flags) = Cr3::read();
            if curr_cr3 != self.cr3 {
                Cr3::write(self.cr3, flags);
            }
        }
    }

    /// Destroys this address space, deallocating all owned physical frames back to the global allocator.
    pub fn destroy(self) {
        drop(self);
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        // If CPU is currently running on this address space, restore kernel CR3 before deallocating
        let (curr_cr3, _) = Cr3::read();
        if curr_cr3 == self.cr3 {
            restore_kernel_address_space();
        }

        for slot in self.owned_frames.iter_mut() {
            if let Some(frame) = slot.take() {
                let _ = deallocate_frame(frame);
            }
        }
        self.owned_count = 0;
    }
}

/// Restores the primary kernel Level 4 Page Table in CR3.
pub fn restore_kernel_address_space() {
    if let Some(kernel_l4) = *KERNEL_L4_FRAME.lock() {
        unsafe {
            let (curr_cr3, flags) = Cr3::read();
            if curr_cr3 != kernel_l4 {
                Cr3::write(kernel_l4, flags);
            }
        }
    }
}

/// Reads the currently active CR3 physical frame.
pub fn current_cr3() -> PhysFrame {
    let (cr3, _) = Cr3::read();
    cr3
}

/// Maximum length permitted for a single sys_write system call (4 KiB = 1 page).
pub const MAX_SYS_WRITE_LEN: u64 = 4096;

/// Validates that a user-provided buffer range [start .. start + len) is:
/// 1. Non-empty and length does not exceed MAX_SYS_WRITE_LEN.
/// 2. Strictly non-null, within canonical user-space (bits 48..63 are 0).
/// 3. Does not overflow u64.
/// 4. Every 4 KiB page spanned by the buffer is mapped, PRESENT, and USER_ACCESSIBLE
///    in the active hardware Level 4 Page Table hierarchy.
pub fn is_user_buffer_valid(start: u64, len: u64) -> bool {
    if len == 0 || len > MAX_SYS_WRITE_LEN || start == 0 {
        return false;
    }

    let end = match start.checked_add(len) {
        Some(e) => e,
        None => return false,
    };

    // User address space upper limit (must be strictly in the lower canonical half)
    if end > 0x0000_7fff_ffff_ffff {
        return false;
    }

    // Read active CR3
    let (cr3_frame, _) = Cr3::read();
    let l4_virt = phys_to_virt(cr3_frame.start_address());
    let l4_table = unsafe { &*(l4_virt.as_ptr() as *const PageTable) };

    // Compute first and last page virtual addresses (4 KiB page aligned)
    let start_page = start & !0xfff;
    let end_page = (end - 1) & !0xfff;

    let mut current_page = start_page;
    loop {
        let p4_idx = ((current_page >> 39) & 0x1ff) as usize;
        let p3_idx = ((current_page >> 30) & 0x1ff) as usize;
        let p2_idx = ((current_page >> 21) & 0x1ff) as usize;
        let p1_idx = ((current_page >> 12) & 0x1ff) as usize;

        // Level 4
        let l4_entry = &l4_table[p4_idx];
        if !l4_entry.flags().contains(PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE) {
            return false;
        }

        // Level 3
        let l3_virt = phys_to_virt(l4_entry.addr());
        let l3_table = unsafe { &*(l3_virt.as_ptr() as *const PageTable) };
        let l3_entry = &l3_table[p3_idx];
        if !l3_entry.flags().contains(PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE) {
            return false;
        }

        // Level 2
        let l2_virt = phys_to_virt(l3_entry.addr());
        let l2_table = unsafe { &*(l2_virt.as_ptr() as *const PageTable) };
        let l2_entry = &l2_table[p2_idx];
        if !l2_entry.flags().contains(PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE) {
            return false;
        }

        // Level 1
        let l1_virt = phys_to_virt(l2_entry.addr());
        let l1_table = unsafe { &*(l1_virt.as_ptr() as *const PageTable) };
        let l1_entry = &l1_table[p1_idx];
        if !l1_entry.flags().contains(PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE) {
            return false;
        }

        if current_page == end_page {
            break;
        }
        match current_page.checked_add(4096) {
            Some(next) => current_page = next,
            None => return false,
        }
    }

    true
}

/// Returns true if the address lies strictly within the valid User Mode virtual range.
#[inline(always)]
pub fn is_valid_user_range(start: u64, len: u64) -> bool {
    if len == 0 || start < USER_SPACE_MIN {
        return false;
    }
    match start.checked_add(len) {
        Some(end) => end <= USER_SPACE_MAX,
        None => false,
    }
}
