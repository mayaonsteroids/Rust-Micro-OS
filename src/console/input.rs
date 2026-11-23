//! Dual Console Input Subsystem: Serial COM1 (0x3F8) and PS/2 Keyboard (0x60).
//!
//! Provides non-blocking polling and line-editing primitives for the interactive shell,
//! supporting ASCII characters, backspace deletion, and carriage-return / newline normalization.

use core::sync::atomic::{AtomicBool, Ordering};
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::port::Port;

/// Global PS/2 Keyboard state decoder configured for US 104-Key layout and Scancode Set 1.
pub static KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
    Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::MapLettersToUnicode)
);

/// State tracker to suppress trailing `\n` in `\r\n` sequences from serial terminals.
static LAST_WAS_CR: AtomicBool = AtomicBool::new(false);

/// Filters and normalizes incoming characters (converts CR to LF, collapses CRLF, maps DEL to BS).
fn filter_input(byte: u8) -> Option<u8> {
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

/// Attempts to read a single byte from either Serial COM1 or the PS/2 Keyboard without blocking.
///
/// Returns `Some(byte)` if an input character is available, or `None` if no input is ready.
pub fn try_read_byte() -> Option<u8> {
    // 1. Poll Serial COM1 (0x3F8)
    let serial_byte = x86_64::instructions::interrupts::without_interrupts(|| {
        crate::console::serial::SERIAL1.lock().try_receive().ok()
    });

    if let Some(b) = serial_byte {
        if let Some(filtered) = filter_input(b) {
            return Some(filtered);
        }
    }

    // 2. Poll PS/2 Keyboard Controller (Port 0x64 status / Port 0x60 data)
    let ps2_byte = x86_64::instructions::interrupts::without_interrupts(|| {
        let mut status_port = Port::<u8>::new(0x64);
        let status = unsafe { status_port.read() };
        // Bit 0 = Output Buffer Full (data available at port 0x60)
        if (status & 1) != 0 {
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

    None
}

/// Reads a single byte from the console, spinning until an input character arrives.
pub fn read_byte() -> u8 {
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
