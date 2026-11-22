//! Intel 8253/8254 Programmable Interval Timer (PIT) Driver.
//!
//! Configures Channel 0 of the PIT to generate periodic timer interrupts at 100 Hz (10 ms period).

use x86_64::instructions::port::Port;

/// The base input clock oscillator frequency of the PIT in Hertz (1.193182 MHz).
pub const PIT_BASE_FREQUENCY_HZ: u32 = 1_193_182;

/// The target timer interrupt frequency for preemptive scheduling (100 Hz = 10 ms per tick).
pub const TARGET_FREQUENCY_HZ: u32 = 100;

/// The calculated divisor for 100 Hz periodic interrupts.
///
/// Divisor = 1,193,182 / 100 = 11,931.82 -> rounded to 11,932 (0x2E9C).
/// Actual frequency: 1,193,182 / 11,932 = 99.99849 Hz (error < 0.002%).
pub const PIT_DIVISOR: u16 = ((PIT_BASE_FREQUENCY_HZ + (TARGET_FREQUENCY_HZ / 2)) / TARGET_FREQUENCY_HZ) as u16;

/// I/O Port for PIT Channel 0 data (read/write reload counter).
pub const PIT_CHANNEL_0_DATA_PORT: u16 = 0x40;

/// I/O Port for PIT Mode/Command Register (write only).
pub const PIT_COMMAND_PORT: u16 = 0x43;

/// PIT Command Byte:
/// - Bits 7-6: `00` = Channel 0
/// - Bits 5-4: `11` = Access mode: lobyte/hibyte
/// - Bits 3-1: `010` = Mode 2 (Rate Generator: periodic pulse on every count)
/// - Bit 0:    `0` = 16-bit binary counter
pub const PIT_COMMAND_MODE_2: u8 = 0b0011_0100; // 0x34

/// Initializes the 8253/8254 PIT to generate hardware IRQ 0 interrupts at 100 Hz.
pub fn init() {
    unsafe {
        let mut command_port = Port::<u8>::new(PIT_COMMAND_PORT);
        let mut data_port = Port::<u8>::new(PIT_CHANNEL_0_DATA_PORT);

        // 1. Send Mode 2 rate-generator command to Channel 0
        command_port.write(PIT_COMMAND_MODE_2);

        // 2. Write low byte of 16-bit divisor
        let low_byte = (PIT_DIVISOR & 0xFF) as u8;
        data_port.write(low_byte);

        // 3. Write high byte of 16-bit divisor
        let high_byte = ((PIT_DIVISOR >> 8) & 0xFF) as u8;
        data_port.write(high_byte);
    }
}
