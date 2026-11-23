//! Structured kernel logging subsystem.
//!
//! Provides level-filtered diagnostic logging with automatic color coding
//! across both VGA display and 16550 serial output.

use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use crate::console::vga::{self, Color};
use crate::{serial_println, vga_print, vga_println};

/// Severity levels for kernel diagnostic messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl LogLevel {
    /// Return the textual severity label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    /// Return the designated VGA foreground color for this level.
    pub const fn color(&self) -> Color {
        match self {
            LogLevel::Debug => Color::LightGray,
            LogLevel::Info => Color::LightCyan,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::LightRed,
        }
    }
}

/// Active global minimum logging level (default: Debug).
/// Any message below this level is filtered out.
static MIN_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Debug as u8);

/// Flag controlling whether diagnostic log messages are printed to the VGA display.
/// Defaults to false so the user interface remains clean, while serial logging
/// preserves all diagnostic messages.
static VGA_LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables or disables VGA screen output for diagnostic logging.
pub fn enable_vga_logging(enable: bool) {
    VGA_LOGGING_ENABLED.store(enable, Ordering::SeqCst);
}

/// Checks whether VGA screen output is currently enabled for diagnostic logging.
pub fn is_vga_logging_enabled() -> bool {
    VGA_LOGGING_ENABLED.load(Ordering::SeqCst)
}

/// Sets the minimum logging severity threshold.
pub fn set_min_level(level: LogLevel) {
    MIN_LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

/// Retrieves the current minimum logging severity threshold.
pub fn get_min_level() -> LogLevel {
    match MIN_LOG_LEVEL.load(Ordering::Relaxed) {
        0 => LogLevel::Debug,
        1 => LogLevel::Info,
        2 => LogLevel::Warn,
        _ => LogLevel::Error,
    }
}

#[doc(hidden)]
pub fn _log(level: LogLevel, args: fmt::Arguments) {
    // Filter messages below the configured minimum level
    if (level as u8) < MIN_LOG_LEVEL.load(Ordering::Relaxed) {
        return;
    }

    use x86_64::instructions::interrupts;

    // Output to both VGA and Serial with interrupt safety
    interrupts::without_interrupts(|| {
        // 1. Output to Serial terminal (always preserved for diagnostics and test suites)
        serial_println!("[{}] {}", level.as_str(), args);

        // 2. Output to VGA screen only if VGA logging is explicitly enabled
        if VGA_LOGGING_ENABLED.load(Ordering::Relaxed) {
            vga::set_color(level.color(), Color::Black);
            vga_print!("[{}] ", level.as_str());
            vga::set_color(Color::White, Color::Black);
            vga_println!("{}", args);
        }
    });
}

/// Logs an informational message in Light Cyan.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::console::logger::_log($crate::console::logger::LogLevel::Info, format_args!($($arg)*));
    };
}

/// Logs a warning message in Yellow.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::console::logger::_log($crate::console::logger::LogLevel::Warn, format_args!($($arg)*));
    };
}

/// Logs an error message in Light Red.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::console::logger::_log($crate::console::logger::LogLevel::Error, format_args!($($arg)*));
    };
}

/// Logs a debug diagnostic message in Light Gray.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::console::logger::_log($crate::console::logger::LogLevel::Debug, format_args!($($arg)*));
    };
}
