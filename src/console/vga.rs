//! VGA Text Buffer Driver for 80x25 character mode.
//!
//! Provides screen clearing, colored text rendering, automatic line wrapping,
//! scrolling, and safe synchronized access to physical MMIO address 0xb8000.

use core::fmt;
use spin::Mutex;
use volatile::Volatile;

/// Standard 4-bit VGA color codes.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Represents an 8-bit VGA color attribute (4 bits background, 4 bits foreground).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    /// Creates a color code combining foreground and background.
    pub const fn new(foreground: Color, background: Color) -> Self {
        Self((background as u8) << 4 | (foreground as u8))
    }
}

/// Represents a single character cell on the screen (ASCII byte + Color attribute byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ScreenChar {
    pub ascii_character: u8,
    pub color_code: ColorCode,
}

/// Number of rows in standard VGA text mode.
pub const BUFFER_HEIGHT: usize = 25;
/// Number of columns in standard VGA text mode.
pub const BUFFER_WIDTH: usize = 80;

/// Represents the 80x25 Memory-Mapped VGA text buffer at physical address 0xb8000.
#[repr(transparent)]
pub struct Buffer {
    pub chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// The stateful VGA Writer managing cursor position and screen writes.
pub struct Writer {
    pub column_position: usize,
    pub color_code: ColorCode,
}

impl Writer {
    /// Creates a new Writer positioned at the start with default colors.
    pub const fn new() -> Self {
        Self {
            column_position: 0,
            color_code: ColorCode::new(Color::White, Color::Black),
        }
    }

    /// Access the memory-mapped VGA text buffer at physical address 0xb8000.
    #[inline(always)]
    fn buffer(&mut self) -> &mut Buffer {
        // SAFETY: 0xb8000 is the standard VGA text buffer address, identity mapped in Long Mode.
        unsafe { &mut *(0xb8000 as *mut Buffer) }
    }

    /// Writes a single byte to the VGA text buffer.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\x08' => {
                if self.column_position > 0 {
                    self.column_position -= 1;
                    let row = BUFFER_HEIGHT - 1;
                    let col = self.column_position;
                    let blank = ScreenChar {
                        ascii_character: b' ',
                        color_code: self.color_code,
                    };
                    self.buffer().chars[row][col].write(blank);
                }
            }
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer().chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    /// Writes an ASCII string to the VGA buffer.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII byte, newline, or backspace
                0x20..=0x7e | b'\n' | b'\x08' => self.write_byte(byte),
                // Unhandled characters printed as a solid block (0xfe)
                _ => self.write_byte(0xfe),
            }
        }
    }

    /// Sets the active text colors (foreground and background).
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }

    /// Moves all lines up by one row and clears the bottom row (scrolling).
    pub fn new_line(&mut self) {
        let buffer = self.buffer();
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = buffer.chars[row][col].read();
                buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Clears a row by filling it with blank space characters.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        let buffer = self.buffer();
        for col in 0..BUFFER_WIDTH {
            buffer.chars[row][col].write(blank);
        }
    }

    /// Clears the entire 80x25 screen.
    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Global synchronized VGA Writer wrapped in a spinlock.
pub static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

/// Configures the VGA Attribute Controller and DAC Palette for the Light Lavender aesthetic theme.
///
/// Palette specification:
/// - Background (0 / Black): Very Light Lavender (#E9DFFF) -> (58, 55, 63)
/// - Normal text (15 / White): Dark Plum (#38264F) -> (14, 9, 20)
/// - Headings / Accents (11 / LightCyan): Medium Purple (#7952A8) -> (30, 20, 42)
/// - Title Bar / Button Bg (1 / Blue): Medium Purple (#7952A8) -> (30, 20, 42)
/// - Window Borders / Frames (3 / Cyan): Deep Purple (#5C3C8A) -> (23, 15, 34)
/// - Window Surfaces (7 / LightGray): Elevated Light Lavender White (#F8F5FF) -> (62, 61, 63)
/// - Subtle borders / Inactive (8 / DarkGray): Lavender Slate (#6B5B82) -> (26, 22, 32)
/// - Success text (10 / LightGreen, 2 / Green): Muted Green (#2D6A4F) -> (11, 26, 20)
/// - Warnings (14 / Yellow): Soft Rose (#9B3356) -> (38, 13, 21)
/// - Errors / Alerts (12 / LightRed, 4 / Red): Readable Dark Red (#8A1C14) -> (34, 7, 5)
/// - High Contrast Labels (13 / Pink): Pure White (#FFFFFF) -> (63, 63, 63)
pub fn init_lavender_palette() {
    use x86_64::instructions::port::Port;

    unsafe {
        // 1. Reset Attribute Controller flip-flop and map attributes 0..15 to DAC indices 0..15
        let mut status_port = Port::<u8>::new(0x3DA);
        let mut atc_port = Port::<u8>::new(0x3C0);

        for i in 0..16u8 {
            let _ = status_port.read();
            atc_port.write(i); // select attribute register i
            atc_port.write(i); // map to DAC color register i
        }

        // Re-enable video display (bit 5 = 1)
        let _ = status_port.read();
        atc_port.write(0x20);

        // 2. Program VGA DAC Palette Registers (Port 0x3C8 write index, Port 0x3C9 RGB data)
        // Values are 6-bit (0..63)
        let mut dac_index = Port::<u8>::new(0x3C8);
        let mut dac_data = Port::<u8>::new(0x3C9);

        // Palette mapping: (Index, R, G, B)
        let palette: [(u8, u8, u8, u8); 16] = [
            (0, 58, 55, 63),  // 0: Black -> Desktop Background: Very Light Lavender (#E9DFFF)
            (1, 30, 20, 42),  // 1: Blue -> Purple Title Bar & Accent Background (#7952A8)
            (2, 11, 26, 20),  // 2: Green -> Muted Green Button Background (#2D6A4F)
            (3, 23, 15, 34),  // 3: Cyan -> Deep Purple Window Frames & Lines (#5C3C8A)
            (4, 34, 7, 5),    // 4: Red -> Readable Dark Red Button Surface (#8A1C14)
            (5, 19, 10, 21),  // 5: Magenta -> Deep Plum (#4E2856)
            (6, 40, 19, 30),  // 6: Brown -> Soft Berry Rose (#A24D7A)
            (7, 62, 61, 63),  // 7: LightGray -> Window Surface: Elevated Light Lavender White (#F8F5FF)
            (8, 26, 22, 32),  // 8: DarkGray -> Lavender Slate (#6B5B82)
            (9, 27, 22, 38),  // 9: LightBlue -> Soft Violet (#6C5B99)
            (10, 11, 26, 20), // 10: LightGreen -> Muted Green (Success #2D6A4F)
            (11, 30, 20, 42), // 11: LightCyan -> Medium Purple (Headings / INFO #7952A8)
            (12, 34, 7, 5),   // 12: LightRed -> Readable Dark Red (Error #8A1C14)
            (13, 63, 63, 63), // 13: Pink -> Pure White (#FFFFFF)
            (14, 38, 13, 21), // 14: Yellow -> Soft Rose (Warnings #9B3356)
            (15, 14, 9, 20),  // 15: White -> Normal text: Dark Plum (#38264F)
        ];

        for (idx, r, g, b) in palette {
            dac_index.write(idx);
            dac_data.write(r);
            dac_data.write(g);
            dac_data.write(b);
        }

        // Program legacy DAC mappings (20, 56..63) for hardware that bypasses ATC
        dac_index.write(20);
        dac_data.write(26);
        dac_data.write(20);
        dac_data.write(16);

        for i in 8..=15u8 {
            let legacy_dac = 48 + i;
            let (_, r, g, b) = palette[i as usize];
            dac_index.write(legacy_dac);
            dac_data.write(r);
            dac_data.write(g);
            dac_data.write(b);
        }
    }
}

/// Initializes the VGA screen by programming the lavender palette and clearing it.
pub fn init() {
    init_lavender_palette();
    let mut writer = WRITER.lock();
    writer.set_color(Color::White, Color::Black);
    writer.clear_screen();
}

/// Clears the VGA screen.
pub fn clear_screen() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().clear_screen();
    });
}

/// Sets the global VGA writer color.
pub fn set_color(foreground: Color, background: Color) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER.lock().set_color(foreground, background);
    });
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

/// Prints formatted text directly to the VGA text screen.
#[macro_export]
macro_rules! vga_print {
    ($($arg:tt)*) => {
        $crate::console::vga::_print(format_args!($($arg)*));
    };
}

/// Prints formatted text directly to the VGA text screen with a newline.
#[macro_export]
macro_rules! vga_println {
    () => ($crate::vga_print!("\n"));
    ($fmt:expr) => ($crate::vga_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::vga_print!(concat!($fmt, "\n"), $($arg)*));
}
