//! Dual Console Input Subsystem: Serial COM1 (0x3F8) and PS/2 Keyboard (0x60).
//!
//! Provides interrupt-driven and polled input primitives for the interactive shell,
//! supporting ASCII characters, backspace deletion, and carriage-return / newline normalization.

use core::sync::atomic::{AtomicBool, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

/// Circular ring buffer for buffering decoded keyboard input bytes.
pub struct KeyBuffer {
    buffer: [u8; 128],
    head: usize,
    tail: usize,
}

impl KeyBuffer {
    pub const fn new() -> Self {
        Self {
            buffer: [0; 128],
            head: 0,
            tail: 0,
        }
    }

    /// Pushes a byte into the ring buffer. Returns true if queued, false if full.
    pub fn push(&mut self, byte: u8) -> bool {
        let next = (self.head + 1) % self.buffer.len();
        if next != self.tail {
            self.buffer[self.head] = byte;
            self.head = next;
            true
        } else {
            false
        }
    }

    /// Pops a byte from the ring buffer, if available.
    pub fn pop(&mut self) -> Option<u8> {
        if self.head != self.tail {
            let byte = self.buffer[self.tail];
            self.tail = (self.tail + 1) % self.buffer.len();
            Some(byte)
        } else {
            None
        }
    }

    /// Returns true if the buffer has no queued bytes.
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// Returns the number of queued bytes in the buffer.
    pub fn len(&self) -> usize {
        if self.head >= self.tail {
            self.head - self.tail
        } else {
            self.buffer.len() - (self.tail - self.head)
        }
    }
}

/// Global thread-safe ring buffer for keyboard input.
pub static KEY_BUFFER: Mutex<KeyBuffer> = Mutex::new(KeyBuffer::new());

/// Global PS/2 Keyboard state decoder configured for US 104-Key layout and Scancode Set 1.
pub static KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
    Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::MapLettersToUnicode)
);

/// State tracker to suppress trailing `\n` in `\r\n` sequences from serial terminals.
static LAST_WAS_CR: AtomicBool = AtomicBool::new(false);

/// Initializes the 8042 PS/2 controller and keyboard hardware for reliable interrupt delivery.
///
/// Flushes any stale bytes from the output buffer, enables the first PS/2 port,
/// configures the controller command byte (enabling IRQ 1, clock, and translation),
/// and performs a clean secondary flush.
pub fn init() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut status_port = Port::<u8>::new(0x64);
        let mut data_port = Port::<u8>::new(0x60);

        // 1. Flush any stale bytes from the output buffer
        for _ in 0..100 {
            if (unsafe { status_port.read() } & 1) != 0 {
                let _ = unsafe { data_port.read() };
            } else {
                break;
            }
        }

        // 2. Enable First PS/2 Port (Command 0xAE)
        unsafe { status_port.write(0xAE); }

        // 3. Read Controller Configuration Byte (Command 0x20)
        unsafe { status_port.write(0x20); }
        for _ in 0..10_000 {
            if (unsafe { status_port.read() } & 1) != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        let mut config = unsafe { data_port.read() };

        // Bit 0 = 1: Enable IRQ 1 for first PS/2 port (Keyboard)
        config |= 1 << 0;
        // Bit 4 = 0: Enable first PS/2 port clock
        config &= !(1 << 4);
        // Bit 6 = 1: Enable translation (Scancode Set 2 to Set 1)
        config |= 1 << 6;

        // 4. Write modified Controller Configuration Byte (Command 0x60)
        unsafe { status_port.write(0x60); }
        for _ in 0..10_000 {
            if (unsafe { status_port.read() } & 2) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        unsafe { data_port.write(config); }

        // 5. Final flush
        for _ in 0..100 {
            if (unsafe { status_port.read() } & 1) != 0 {
                let _ = unsafe { data_port.read() };
            } else {
                break;
            }
        }
    });

    crate::info!("[KEYBOARD] PS/2 8042 controller initialized (IRQ1 unmasked, translation enabled).");
}

/// Keyboard interrupt handler registered at vector 33 (IRQ 1).
///
/// Reads the scancode from port 0x60, decodes make/break scancodes using `KEYBOARD`,
/// enqueues valid characters into `KEY_BUFFER`, and acknowledges the interrupt via PIC EOI.
pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut status_port = Port::<u8>::new(0x64);
    let status = unsafe { status_port.read() };

    // Bit 0 = Output Buffer Full (data available at port 0x60)
    if (status & 1) != 0 {
        let mut data_port = Port::<u8>::new(0x60);
        let scancode = unsafe { data_port.read() };

        // Bit 5 = Mouse Output Buffer (ignore mouse packets if any)
        if (status & 0x20) == 0 {
            let mut keyboard = KEYBOARD.lock();
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(c) => {
                            if c == '\n' || c == '\r' {
                                KEY_BUFFER.lock().push(b'\n');
                            } else if c == '\x08' {
                                KEY_BUFFER.lock().push(b'\x08');
                            } else if c.is_ascii() {
                                KEY_BUFFER.lock().push(c as u8);
                            }
                        }
                        DecodedKey::RawKey(KeyCode::Backspace) => {
                            KEY_BUFFER.lock().push(b'\x08');
                        }
                        DecodedKey::RawKey(KeyCode::Return) => {
                            KEY_BUFFER.lock().push(b'\n');
                        }
                        DecodedKey::RawKey(KeyCode::Escape) => {
                            KEY_BUFFER.lock().push(b'\x1b');
                        }
                        DecodedKey::RawKey(KeyCode::ArrowUp) => {
                            KEY_BUFFER.lock().push(0x1E); // RS - Up Arrow
                        }
                        DecodedKey::RawKey(KeyCode::ArrowDown) => {
                            KEY_BUFFER.lock().push(0x1F); // US - Down Arrow
                        }
                        DecodedKey::RawKey(KeyCode::Numpad1) => { KEY_BUFFER.lock().push(b'1'); }
                        DecodedKey::RawKey(KeyCode::Numpad2) => { KEY_BUFFER.lock().push(b'2'); }
                        DecodedKey::RawKey(KeyCode::Numpad3) => { KEY_BUFFER.lock().push(b'3'); }
                        DecodedKey::RawKey(KeyCode::Numpad4) => { KEY_BUFFER.lock().push(b'4'); }
                        DecodedKey::RawKey(KeyCode::Numpad5) => { KEY_BUFFER.lock().push(b'5'); }
                        DecodedKey::RawKey(KeyCode::Numpad6) => { KEY_BUFFER.lock().push(b'6'); }
                        DecodedKey::RawKey(KeyCode::Numpad7) => { KEY_BUFFER.lock().push(b'7'); }
                        DecodedKey::RawKey(KeyCode::Numpad8) => { KEY_BUFFER.lock().push(b'8'); }
                        DecodedKey::RawKey(KeyCode::NumpadAdd) => { KEY_BUFFER.lock().push(b'+'); }
                        DecodedKey::RawKey(KeyCode::NumpadSubtract) => { KEY_BUFFER.lock().push(b'-'); }
                        _ => {}
                    }
                }
            }
        }
    }

    // Acknowledge interrupt by sending EOI to Master PIC
    crate::interrupts::pic::send_eoi(crate::interrupts::pic::KEYBOARD_INTERRUPT_ID);
}

/// Filters and normalizes incoming characters (converts CR to LF, collapses CRLF, maps DEL to BS).
pub fn filter_input(byte: u8) -> Option<u8> {
    if byte == b'\r' {
        LAST_WAS_CR.store(true, Ordering::SeqCst);
        Some(b'\n')
    } else if byte == b'\n' {
        if LAST_WAS_CR.swap(false, Ordering::SeqCst) {
            // Suppress redundant newline from CRLF pair
            None
        } else {
            Some(b'\n')
        }
    } else {
        LAST_WAS_CR.store(false, Ordering::SeqCst);
        if byte == 0x7F {
            Some(b'\x08')
        } else {
            Some(byte)
        }
    }
}

/// Attempts to read a single byte from either the interrupt key buffer, Serial COM1, or PS/2 fallback.
///
/// Returns `Some(byte)` if an input character is available, or `None` if no input is ready.
pub fn try_read_byte() -> Option<u8> {
    // 1. Check interrupt-buffered keyboard characters
    let buffered_byte = x86_64::instructions::interrupts::without_interrupts(|| {
        KEY_BUFFER.lock().pop()
    });

    if let Some(b) = buffered_byte {
        if let Some(filtered) = filter_input(b) {
            return Some(filtered);
        }
    }

    // 2. Poll Serial COM1 (0x3F8)
    let serial_byte = x86_64::instructions::interrupts::without_interrupts(|| {
        crate::console::serial::SERIAL1.lock().try_receive().ok()
    });

    if let Some(b) = serial_byte {
        if let Some(filtered) = filter_input(b) {
            return Some(filtered);
        }
    }

    // 3. Fallback: Direct poll PS/2 Keyboard Controller ONLY when CPU interrupts are disabled
    if !x86_64::instructions::interrupts::are_enabled() {
        let ps2_byte = x86_64::instructions::interrupts::without_interrupts(|| {
            let mut status_port = Port::<u8>::new(0x64);
            let status = unsafe { status_port.read() };
            // Bit 0 = Output Buffer Full, Bit 5 = Not Mouse
            if (status & 1) != 0 && (status & 0x20) == 0 {
                let mut data_port = Port::<u8>::new(0x60);
                let scancode = unsafe { data_port.read() };
                let mut keyboard = KEYBOARD.lock();
                if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                    if let Some(key) = keyboard.process_keyevent(key_event) {
                        match key {
                            DecodedKey::Unicode(c) => {
                                if c == '\n' || c == '\r' {
                                    return Some(b'\n');
                                } else if c == '\x08' {
                                    return Some(b'\x08');
                                } else if c.is_ascii() {
                                    return Some(c as u8);
                                }
                            }
                            DecodedKey::RawKey(KeyCode::Backspace) => return Some(b'\x08'),
                            DecodedKey::RawKey(KeyCode::Return) => return Some(b'\n'),
                            DecodedKey::RawKey(KeyCode::Escape) => return Some(b'\x1b'),
                            DecodedKey::RawKey(KeyCode::Numpad1) => return Some(b'1'),
                            DecodedKey::RawKey(KeyCode::Numpad2) => return Some(b'2'),
                            DecodedKey::RawKey(KeyCode::Numpad3) => return Some(b'3'),
                            DecodedKey::RawKey(KeyCode::Numpad4) => return Some(b'4'),
                            DecodedKey::RawKey(KeyCode::Numpad5) => return Some(b'5'),
                            DecodedKey::RawKey(KeyCode::Numpad6) => return Some(b'6'),
                            DecodedKey::RawKey(KeyCode::Numpad7) => return Some(b'7'),
                            DecodedKey::RawKey(KeyCode::Numpad8) => return Some(b'8'),
                            DecodedKey::RawKey(KeyCode::NumpadAdd) => return Some(b'+'),
                            DecodedKey::RawKey(KeyCode::NumpadSubtract) => return Some(b'-'),
                            _ => {}
                        }
                    }
                }
            }
            None
        });

        if let Some(b) = ps2_byte {
            if let Some(filtered) = filter_input(b) {
                return Some(filtered);
            }
        }
    }

    None
}

/// Reads a single byte from the console, spinning until an input character arrives.
/// Automatically ensures CPU interrupts are enabled while waiting.
pub fn read_byte() -> u8 {
    if !x86_64::instructions::interrupts::are_enabled() {
        x86_64::instructions::interrupts::enable();
    }
    loop {
        if let Some(b) = try_read_byte() {
            return b;
        }
        core::hint::spin_loop();
    }
}

/// Reads an entire line from the console into `buf`, supporting character echo and backspace.
///
/// Terminates when newline (`\n` or `\r`) is entered, returning the byte length of the line.
pub fn read_line(buf: &mut [u8]) -> usize {
    let mut len = 0;
    loop {
        let b = read_byte();
        match b {
            b'\n' => {
                crate::kprintln!();
                return len;
            }
            b'\x08' => {
                if len > 0 {
                    len -= 1;
                    crate::kprint!("\x08");
                }
            }
            byte if (0x20..=0x7E).contains(&byte) => {
                if len < buf.len() {
                    buf[len] = byte;
                    len += 1;
                    crate::kprint!("{}", byte as char);
                }
            }
            _ => {}
        }
    }
}

/// Unit tests for PS/2 keyboard scancode processing and ring buffer operations.
pub mod tests {
    use super::*;
    use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};

    /// Tests make and break scancode decoding for standard keys, enter, backspace, and shift.
    pub fn run_keyboard_unit_tests() -> bool {
        let mut all_pass = true;

        // 1. Test KeyBuffer ring buffer FIFO behavior
        let mut buf = KeyBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.push(b'H'));
        assert!(buf.push(b'e'));
        assert!(buf.push(b'y'));
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.pop(), Some(b'H'));
        assert_eq!(buf.pop(), Some(b'e'));
        assert_eq!(buf.pop(), Some(b'y'));
        assert_eq!(buf.pop(), None);
        assert!(buf.is_empty());

        // 2. Test Scancode Set 1 make and break processing
        let mut kb = Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::MapLettersToUnicode);

        // 'h' key (make code = 0x23, break code = 0xA3)
        let ev1 = kb.add_byte(0x23).unwrap().unwrap();
        assert_eq!(ev1.state, pc_keyboard::KeyState::Down);
        let key1 = kb.process_keyevent(ev1);
        assert_eq!(key1, Some(DecodedKey::Unicode('h')));

        let ev1_up = kb.add_byte(0xA3).unwrap().unwrap();
        assert_eq!(ev1_up.state, pc_keyboard::KeyState::Up);
        let key1_up = kb.process_keyevent(ev1_up);
        assert_eq!(key1_up, None); // Key release produces no character

        // Enter key (make code = 0x1C, break code = 0x9C)
        let ev_enter = kb.add_byte(0x1C).unwrap().unwrap();
        assert_eq!(ev_enter.state, pc_keyboard::KeyState::Down);
        let key_enter = kb.process_keyevent(ev_enter);
        match key_enter {
            Some(DecodedKey::Unicode('\n')) | Some(DecodedKey::RawKey(KeyCode::Return)) => {}
            other => {
                crate::error!("[KEY-TEST] Expected Return/Newline, got: {:?}", other);
                all_pass = false;
            }
        }
        let ev_enter_up = kb.add_byte(0x9C).unwrap().unwrap();
        assert_eq!(ev_enter_up.state, pc_keyboard::KeyState::Up);
        assert_eq!(kb.process_keyevent(ev_enter_up), None);

        // Backspace key (make code = 0x0E, break code = 0x8E)
        let ev_bs = kb.add_byte(0x0E).unwrap().unwrap();
        assert_eq!(ev_bs.state, pc_keyboard::KeyState::Down);
        let key_bs = kb.process_keyevent(ev_bs);
        match key_bs {
            Some(DecodedKey::Unicode('\x08')) | Some(DecodedKey::RawKey(KeyCode::Backspace)) => {}
            other => {
                crate::error!("[KEY-TEST] Expected Backspace, got: {:?}", other);
                all_pass = false;
            }
        }
        let ev_bs_up = kb.add_byte(0x8E).unwrap().unwrap();
        assert_eq!(ev_bs_up.state, pc_keyboard::KeyState::Up);
        assert_eq!(kb.process_keyevent(ev_bs_up), None);

        // Left Shift + 'a' -> 'A' (Shift make = 0x2A, 'a' make = 0x1E)
        let ev_shift = kb.add_byte(0x2A).unwrap().unwrap();
        let key_shift = kb.process_keyevent(ev_shift);
        match key_shift {
            Some(DecodedKey::RawKey(KeyCode::LShift)) | None => {}
            other => {
                crate::error!("[KEY-TEST] Expected LShift or None, got: {:?}", other);
                all_pass = false;
            }
        }
        let ev_a = kb.add_byte(0x1E).unwrap().unwrap();
        assert_eq!(kb.process_keyevent(ev_a), Some(DecodedKey::Unicode('A')));

        // Shift release (0xAA)
        let ev_shift_up = kb.add_byte(0xAA).unwrap().unwrap();
        let key_shift_up = kb.process_keyevent(ev_shift_up);
        match key_shift_up {
            Some(DecodedKey::RawKey(KeyCode::LShift)) | None => {}
            other => {
                crate::error!("[KEY-TEST] Expected LShift release or None, got: {:?}", other);
                all_pass = false;
            }
        }

        // 'a' alone -> 'a'
        let ev_a2 = kb.add_byte(0x1E).unwrap().unwrap();
        assert_eq!(kb.process_keyevent(ev_a2), Some(DecodedKey::Unicode('a')));
        let ev_a2_up = kb.add_byte(0x9E).unwrap().unwrap();
        assert_eq!(kb.process_keyevent(ev_a2_up), None);

        // 3. Test filter_input normalization
        assert_eq!(filter_input(b'\r'), Some(b'\n'));
        assert_eq!(filter_input(b'\n'), None); // redundant LF suppressed after CR
        assert_eq!(filter_input(b'\n'), Some(b'\n')); // standalone LF preserved
        assert_eq!(filter_input(0x7F), Some(b'\x08')); // DEL mapped to BS
        assert_eq!(filter_input(b'X'), Some(b'X'));

        all_pass
    }
}
