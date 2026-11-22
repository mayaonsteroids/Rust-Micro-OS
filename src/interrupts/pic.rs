//! 8259 Programmable Interrupt Controller (PIC) management.
//!
//! Remaps hardware IRQs from legacy IBM PC defaults (0x08..0x0F) to vectors 32..47
//! to avoid collisions with standard x86_64 CPU architectural exceptions.

use pic8259::ChainedPics;
use spin::Mutex;

/// Vector offset for Master PIC (IRQs 0..7 map to interrupt vectors 32..39).
pub const PIC_1_OFFSET: u8 = 32;

/// Vector offset for Slave PIC (IRQs 8..15 map to interrupt vectors 40..47).
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8; // 40

/// Interrupt vector assigned to the 8253/8254 PIT hardware timer (IRQ 0).
pub const TIMER_INTERRUPT_ID: u8 = PIC_1_OFFSET; // 32

/// Synchronized dual 8259 PIC controller abstraction.
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// Initializes the 8259 PIC controllers, remaps IRQ offsets, and unmasks IRQ 0 (timer).
pub fn init() {
    unsafe {
        let mut pics = PICS.lock();
        pics.initialize();

        // Configure interrupt masks:
        // Master PIC: Enable IRQ 0 (timer), mask IRQs 1..7 (keyboard, etc.)
        // Port 0x21: mask byte (0 = unmasked/enabled, 1 = masked/disabled)
        // 0b1111_1110 = 0xFE
        let mut master_mask_port = x86_64::instructions::port::Port::<u8>::new(0x21);
        master_mask_port.write(0xFE);

        // Slave PIC: Mask all IRQs 8..15
        // Port 0xA1: 0xFF
        let mut slave_mask_port = x86_64::instructions::port::Port::<u8>::new(0xA1);
        slave_mask_port.write(0xFF);
    }
}

/// Acknowledges an interrupt by sending an End Of Interrupt (EOI) command to the PICs.
#[inline]
pub fn send_eoi(interrupt_id: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(interrupt_id);
    }
}
