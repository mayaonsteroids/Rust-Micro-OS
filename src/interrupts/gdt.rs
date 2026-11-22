//! Global Descriptor Table (GDT), Task State Segment (TSS), and segment selectors.
//!
//! Configures 64-bit kernel and user-mode privilege levels (Ring 0 and Ring 3)
//! required for Milestone 13 hardware protection and system calls.

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// Size of the dedicated kernel interrupt stack for user-mode transitions (8 KiB).
pub const KERNEL_INTERRUPT_STACK_SIZE: usize = 8192;

#[repr(C, align(16))]
struct KernelInterruptStack([u8; KERNEL_INTERRUPT_STACK_SIZE]);

static mut KERNEL_INTERRUPT_STACK: KernelInterruptStack = KernelInterruptStack([0; KERNEL_INTERRUPT_STACK_SIZE]);

/// Global Task State Segment holding the Ring 0 kernel stack pointer (RSP0).
pub static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Segment selectors created during GDT initialization.
#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

impl Selectors {
    pub const fn empty() -> Self {
        Self {
            kernel_code: SegmentSelector::NULL,
            kernel_data: SegmentSelector::NULL,
            user_data: SegmentSelector::NULL,
            user_code: SegmentSelector::NULL,
            tss: SegmentSelector::NULL,
        }
    }
}

/// Global Descriptor Table storage.
pub static mut GDT: (GlobalDescriptorTable, Selectors) = (
    GlobalDescriptorTable::new(),
    Selectors::empty(),
);

/// Initializes and loads the GDT and TSS into the CPU.
pub fn init() {
    unsafe {
        let tss = &mut *core::ptr::addr_of_mut!(TSS);

        // Configure privilege stack 0 (RSP0) used by hardware on privilege transition from Ring 3
        let stack_top = VirtAddr::new(core::ptr::addr_of!(KERNEL_INTERRUPT_STACK.0) as u64) + KERNEL_INTERRUPT_STACK_SIZE as u64;
        tss.privilege_stack_table[0] = stack_top;

        let gdt = &mut (*core::ptr::addr_of_mut!(GDT)).0;
        *gdt = GlobalDescriptorTable::new();

        let kcode = gdt.add_entry(Descriptor::kernel_code_segment());
        let kdata = gdt.add_entry(Descriptor::kernel_data_segment());
        let udata = gdt.add_entry(Descriptor::user_data_segment());
        let ucode = gdt.add_entry(Descriptor::user_code_segment());
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(tss));

        (*core::ptr::addr_of_mut!(GDT)).1 = Selectors {
            kernel_code: kcode,
            kernel_data: kdata,
            user_data: udata,
            user_code: ucode,
            tss: tss_sel,
        };

        gdt.load();

        // Reload segment registers to point to the newly configured descriptors
        use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
        use x86_64::instructions::tables::load_tss;

        CS::set_reg(kcode);
        SS::set_reg(kdata);
        DS::set_reg(kdata);
        ES::set_reg(kdata);
        load_tss(tss_sel);
    }
}

/// Returns the active segment selectors.
pub fn selectors() -> Selectors {
    unsafe { (*core::ptr::addr_of!(GDT)).1 }
}

/// Returns the 64-bit User Code Segment selector (RPL = 3).
pub fn user_code_selector() -> SegmentSelector {
    selectors().user_code
}

/// Returns the User Data Segment selector (RPL = 3).
pub fn user_data_selector() -> SegmentSelector {
    selectors().user_data
}

/// Returns the 64-bit Kernel Code Segment selector (RPL = 0).
pub fn kernel_code_selector() -> SegmentSelector {
    selectors().kernel_code
}

/// Returns the Kernel Data Segment selector (RPL = 0).
pub fn kernel_data_selector() -> SegmentSelector {
    selectors().kernel_data
}

/// Updates the active kernel privilege stack (TSS RSP0).
pub fn set_kernel_stack(stack_top: VirtAddr) {
    unsafe {
        TSS.privilege_stack_table[0] = stack_top;
    }
}

/// Restores the default global kernel interrupt stack in TSS RSP0.
pub fn restore_default_kernel_stack() {
    unsafe {
        let stack_top = VirtAddr::new(core::ptr::addr_of!(KERNEL_INTERRUPT_STACK.0) as u64) + KERNEL_INTERRUPT_STACK_SIZE as u64;
        TSS.privilege_stack_table[0] = stack_top;
    }
}
