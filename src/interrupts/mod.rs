//! Hardware interrupt handling, IDT, GDT, TSS, and 8259 PIC management.

pub mod gdt;
pub mod idt;
pub mod pic;
pub mod syscall;
pub mod tests;

/// Initializes the complete interrupt subsystem (GDT, TSS, PIC, PIT, IDT).
///
/// NOTE: Interrupts remain disabled (`IF=0`) until explicitly enabled via `x86_64::instructions::interrupts::enable()`.
pub fn init() {
    gdt::init();
    pic::init();
    crate::timer::pit::init();
    idt::init();
}
