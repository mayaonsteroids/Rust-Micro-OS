//! Dedicated PS/2 Mouse Driver for BubbleOS.
//!
//! Provides 8042 auxiliary device initialization, bounded polling with timeout protection,
//! 3-byte packet decoding (X/Y delta, sign extension, overflow handling, button states),
//! coordinate clamping (1024x768), edge-triggered click detection, and IRQ 12 interrupt servicing.

use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

/// Clamped screen bounds for the 1024x768 truecolor graphical desktop.
pub const SCREEN_WIDTH: usize = 1024;
pub const SCREEN_HEIGHT: usize = 768;

/// Snapshot of the current mouse pointer state and button status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseState {
    pub x: usize,
    pub y: usize,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub left_clicked: bool,
    pub right_clicked: bool,
    pub is_present: bool,
    pub packet_count: u64,
}

impl MouseState {
    pub const fn new() -> Self {
        Self {
            x: SCREEN_WIDTH / 2,  // 512
            y: SCREEN_HEIGHT / 2, // 384
            left_button: false,
            right_button: false,
            middle_button: false,
            left_clicked: false,
            right_clicked: false,
            is_present: false,
            packet_count: 0,
        }
    }
}

/// Internal driver tracking packet assembly and button transitions.
pub struct MouseDriver {
    pub state: MouseState,
    packet: [u8; 3],
    packet_idx: usize,
    last_left: bool,
    last_right: bool,
}

impl MouseDriver {
    pub const fn new() -> Self {
        Self {
            state: MouseState::new(),
            packet: [0; 3],
            packet_idx: 0,
            last_left: false,
            last_right: false,
        }
    }

    /// Feeds a single byte from the 8042 data port (0x60) into the 3-byte packet parser.
    pub fn process_byte(&mut self, byte: u8) {
        match self.packet_idx {
            0 => {
                // Byte 0 must have Bit 3 set (sync bit). If not, discard and resynchronize.
                if (byte & 0x08) == 0 {
                    return;
                }
                self.packet[0] = byte;
                self.packet_idx = 1;
            }
            1 => {
                self.packet[1] = byte;
                self.packet_idx = 2;
            }
            2 => {
                self.packet[2] = byte;
                self.packet_idx = 0;
                self.decode_packet();
            }
            _ => {
                self.packet_idx = 0;
            }
        }
    }

    /// Decodes a complete 3-byte PS/2 packet into coordinate updates and button transitions.
    fn decode_packet(&mut self) {
        let flags = self.packet[0];
        let raw_dx = self.packet[1];
        let raw_dy = self.packet[2];

        let x_overflow = (flags & 0x40) != 0;
        let y_overflow = (flags & 0x80) != 0;

        let left = (flags & 0x01) != 0;
        let right = (flags & 0x02) != 0;
        let middle = (flags & 0x04) != 0;

        // Discard movement on axis overflow
        if !x_overflow && !y_overflow {
            let dx: i16 = if (flags & 0x10) != 0 {
                (raw_dx as u16 | 0xFF00) as i16
            } else {
                raw_dx as i16
            };

            let dy: i16 = if (flags & 0x20) != 0 {
                (raw_dy as u16 | 0xFF00) as i16
            } else {
                raw_dy as i16
            };

            // In PS/2 mouse protocol, +dy is upward movement.
            // Screen Y coordinates increase downward, so screen_y = y - dy.
            let new_x = (self.state.x as i32 + dx as i32).clamp(0, (SCREEN_WIDTH - 1) as i32) as usize;
            let new_y = (self.state.y as i32 - dy as i32).clamp(0, (SCREEN_HEIGHT - 1) as i32) as usize;

            self.state.x = new_x;
            self.state.y = new_y;
        }

        // Edge-triggered click detection: true on rising edge (false -> true)
        if left && !self.last_left {
            self.state.left_clicked = true;
        }
        if right && !self.last_right {
            self.state.right_clicked = true;
        }

        self.last_left = left;
        self.last_right = right;
        self.state.left_button = left;
        self.state.right_button = right;
        self.state.middle_button = middle;
        self.state.packet_count = self.state.packet_count.wrapping_add(1);
    }
}

/// Global synchronized mouse driver instance.
pub static MOUSE: Mutex<MouseDriver> = Mutex::new(MouseDriver::new());

/// Waits with a bounded timeout for the 8042 input buffer to empty (ready to receive command/data).
fn wait_input_empty() -> bool {
    let mut status_port = Port::<u8>::new(0x64);
    for _ in 0..50_000 {
        if (unsafe { status_port.read() } & 0x02) == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Waits with a bounded timeout for the 8042 output buffer to fill (data ready to be read).
fn wait_output_full() -> bool {
    let mut status_port = Port::<u8>::new(0x64);
    for _ in 0..50_000 {
        if (unsafe { status_port.read() } & 0x01) != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Sends a command byte directly to the auxiliary device (PS/2 mouse).
fn mouse_write(cmd: u8) -> bool {
    let mut status_port = Port::<u8>::new(0x64);
    let mut data_port = Port::<u8>::new(0x60);

    // 0xD4 indicates next byte goes to second PS/2 port
    if !wait_input_empty() {
        return false;
    }
    unsafe { status_port.write(0xD4); }

    if !wait_input_empty() {
        return false;
    }
    unsafe { data_port.write(cmd); }
    true
}

/// Reads a response byte from the mouse with bounded timeout.
fn mouse_read() -> Option<u8> {
    if !wait_output_full() {
        return None;
    }
    let mut data_port = Port::<u8>::new(0x60);
    Some(unsafe { data_port.read() })
}

/// Initializes the 8042 auxiliary port and enables PS/2 mouse packet streaming.
///
/// Bounded loops ensure the kernel never hangs if no mouse hardware is present.
pub fn init() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut status_port = Port::<u8>::new(0x64);
        let mut data_port = Port::<u8>::new(0x60);

        crate::info!("[MOUSE] Initializing 8042 PS/2 auxiliary port...");

        // 1. Flush any stale bytes from output buffer
        for _ in 0..100 {
            if (unsafe { status_port.read() } & 0x01) != 0 {
                let _ = unsafe { data_port.read() };
            } else {
                break;
            }
        }

        // 2. Enable Auxiliary PS/2 Device (Command 0xA8)
        if !wait_input_empty() {
            crate::warn!("[MOUSE] 8042 input buffer busy; cannot enable aux port.");
            return;
        }
        unsafe { status_port.write(0xA8); }

        // 3. Read Controller Configuration Byte (Command 0x20)
        if !wait_input_empty() {
            crate::warn!("[MOUSE] 8042 input buffer busy; cannot read config.");
            return;
        }
        unsafe { status_port.write(0x20); }

        let config = match mouse_read() {
            Some(c) => c,
            None => {
                crate::warn!("[MOUSE] Timeout waiting for 8042 config byte.");
                return;
            }
        };

        // Enable IRQ 12 (Bit 1 = 1) and enable mouse clock (Bit 5 = 0)
        let new_config = (config | (1 << 1)) & !(1 << 5);

        // 4. Write modified Controller Configuration Byte (Command 0x60)
        if !wait_input_empty() {
            crate::warn!("[MOUSE] 8042 input buffer busy; cannot write config cmd.");
            return;
        }
        unsafe { status_port.write(0x60); }

        if !wait_input_empty() {
            crate::warn!("[MOUSE] 8042 input buffer busy; cannot write config data.");
            return;
        }
        unsafe { data_port.write(new_config); }

        // 5. Send Set Defaults (0xF6) to mouse
        if mouse_write(0xF6) {
            let ack = mouse_read();
            if ack != Some(0xFA) {
                crate::warn!("[MOUSE] Set defaults (0xF6) did not return 0xFA ACK (got {:?}).", ack);
            }
        }

        // 6. Send Enable Data Reporting (0xF4) to mouse
        if mouse_write(0xF4) {
            let ack = mouse_read();
            if ack != Some(0xFA) {
                crate::warn!("[MOUSE] Enable reporting (0xF4) did not return 0xFA ACK (got {:?}).", ack);
            }
        }

        // 7. Flush any residual bytes
        for _ in 0..100 {
            if (unsafe { status_port.read() } & 0x01) != 0 {
                let _ = unsafe { data_port.read() };
            } else {
                break;
            }
        }

        // Mark driver as present and ready
        {
            let mut mouse = MOUSE.lock();
            mouse.state.is_present = true;
            mouse.state.x = SCREEN_WIDTH / 2;
            mouse.state.y = SCREEN_HEIGHT / 2;
        }

        crate::info!("[MOUSE] PS/2 Mouse streaming enabled (IRQ 12 active, default 512, 384).");
    });
}

/// Feeds a mouse packet byte from an interrupt handler or multiplexed input router.
pub fn process_mouse_byte(byte: u8) {
    MOUSE.lock().process_byte(byte);
}

/// Retrieves the current mouse snapshot and clears one-shot click flags.
///
/// Guaranteed deadlock-free and interrupt-safe via `without_interrupts`.
pub fn get_mouse_state() -> MouseState {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut mouse = MOUSE.lock();
        let state = mouse.state;
        // Consume one-shot click flags
        mouse.state.left_clicked = false;
        mouse.state.right_clicked = false;
        state
    })
}

/// Returns the current (x, y) coordinates of the mouse pointer.
pub fn get_position() -> (usize, usize) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mouse = MOUSE.lock();
        (mouse.state.x, mouse.state.y)
    })
}

/// Interrupt handler registered at Vector 44 (IRQ 12 - Slave PIC).
pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut status_port = Port::<u8>::new(0x64);
    let status = unsafe { status_port.read() };

    // Bit 0 = Output Buffer Full
    if (status & 0x01) != 0 {
        let mut data_port = Port::<u8>::new(0x60);
        let data = unsafe { data_port.read() };

        // Bit 5 = 1 indicates byte originated from auxiliary mouse device
        if (status & 0x20) != 0 {
            process_mouse_byte(data);
        } else {
            // Forward accidental keyboard scancode received on auxiliary line
            crate::console::input::process_keyboard_byte(data);
        }
    }

    // Acknowledge interrupt by sending EOI to Slave & Master PIC
    crate::interrupts::pic::send_eoi(crate::interrupts::pic::MOUSE_INTERRUPT_ID);
}

pub mod tests {
    use super::*;

    pub fn run_mouse_unit_tests() -> bool {
        crate::info!("--- BEGIN TEST: PS/2 Mouse Unit Tests ---");

        // Test 1: Driver initialization defaults
        let mut driver = MouseDriver::new();
        if driver.state.x != 512 || driver.state.y != 384 {
            crate::error!("Default coordinates mismatch");
            return false;
        }
        if driver.state.left_button || driver.state.right_button {
            crate::error!("Default buttons mismatch");
            return false;
        }

        // Test 2: Sync bit enforcement (Bit 3 must be 1)
        driver.process_byte(0x00);
        if driver.packet_idx != 0 {
            crate::error!("Out-of-sync byte must be dropped");
            return false;
        }

        // Test 3: Positive movement (dx = +10, dy = +5)
        // Byte 0: 0x08 (Sync=1, no sign, no buttons)
        // Byte 1: 10
        // Byte 2: 5 (+dy is upward, so screen y decreases)
        driver.process_byte(0x08);
        driver.process_byte(10);
        driver.process_byte(5);
        if driver.state.x != 522 || driver.state.y != 379 {
            crate::error!("Positive movement failed");
            return false;
        }
        if driver.state.packet_count != 1 {
            crate::error!("Packet count mismatch");
            return false;
        }

        // Test 4: Negative movement with sign extension (dx = -20, dy = -15)
        // Byte 0: 0x08 | 0x10 (X sign) | 0x20 (Y sign) = 0x38
        // dx = -20 -> 256 - 20 = 236 = 0xEC
        // dy = -15 -> 256 - 15 = 241 = 0xF1 (-dy is downward, so screen y increases)
        driver.process_byte(0x38);
        driver.process_byte(0xEC);
        driver.process_byte(0xF1);
        if driver.state.x != 502 || driver.state.y != 394 {
            crate::error!("Negative movement failed");
            return false;
        }
        if driver.state.packet_count != 2 {
            crate::error!("Packet count mismatch");
            return false;
        }

        // Test 5: Clamping at boundaries (X clamping at 1023, Y clamping at 0)
        driver.state.x = 1000;
        driver.process_byte(0x08);
        driver.process_byte(100);
        driver.process_byte(0);
        if driver.state.x != 1023 {
            crate::error!("X clamping at 1023 failed");
            return false;
        }

        driver.state.y = 10;
        driver.process_byte(0x08);
        driver.process_byte(0);
        driver.process_byte(100); // dy = +100 (upward)
        if driver.state.y != 0 {
            crate::error!("Y clamping at 0 failed");
            return false;
        }

        // Test 6: Overflow bits discard movement
        let old_x = driver.state.x;
        let old_y = driver.state.y;
        // Bit 6 = X overflow -> 0x08 | 0x40 = 0x48
        driver.process_byte(0x48);
        driver.process_byte(50);
        driver.process_byte(50);
        if driver.state.x != old_x || driver.state.y != old_y {
            crate::error!("Overflow discard failed");
            return false;
        }

        // Test 7: Button transitions and edge-triggered click detection
        // Press Left button: Byte 0 = 0x08 | 0x01 = 0x09
        driver.process_byte(0x09);
        driver.process_byte(0);
        driver.process_byte(0);
        if !driver.state.left_button || !driver.state.left_clicked {
            crate::error!("Left button press edge detection failed");
            return false;
        }

        // Subsequent packet with Left button still held -> left_clicked should NOT re-trigger
        driver.state.left_clicked = false;
        driver.process_byte(0x09);
        driver.process_byte(0);
        driver.process_byte(0);
        if !driver.state.left_button || driver.state.left_clicked {
            crate::error!("Held button must not re-trigger click");
            return false;
        }

        // Release Left button: Byte 0 = 0x08
        driver.process_byte(0x08);
        driver.process_byte(0);
        driver.process_byte(0);
        if driver.state.left_button || driver.state.left_clicked {
            crate::error!("Left button release failed");
            return false;
        }

        crate::info!("[TEST] All 7 PS/2 Mouse Unit Tests PASSED successfully.");
        crate::info!("--- END TEST: PS/2 Mouse Unit Tests ---");
        true
    }
}
