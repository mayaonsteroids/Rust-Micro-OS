//! Kernel console subsystem combining VGA Text Mode and 16550 Serial UART.

pub mod input;
pub mod logger;
pub mod mouse;
pub mod serial;
pub mod vga;

pub use input::{read_byte, read_line, try_read_byte};
pub use mouse::{get_mouse_state, MouseState};

/// Prints simultaneously to both the VGA screen and the serial console.
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::vga_print!($($arg)*);
        $crate::serial_print!($($arg)*);
    };
}

/// Prints simultaneously to both the VGA screen and the serial console with a newline.
#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::vga_println!();
        $crate::serial_println!();
    };
    ($fmt:expr) => {
        $crate::vga_println!($fmt);
        $crate::serial_println!($fmt);
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::vga_println!($fmt, $($arg)*);
        $crate::serial_println!($fmt, $($arg)*);
    };
}
