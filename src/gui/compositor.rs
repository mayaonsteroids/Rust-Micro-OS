//! Double-Buffered Screen Compositor for 80x25 VGA Text Mode.
//!
//! Provides flicker-free rendering, CP437 box-drawing primitives, clipping,
//! and fast presentation to the physical VGA text buffer at 0xb8000.

use crate::console::vga::{Buffer, Color, ColorCode, ScreenChar, BUFFER_HEIGHT, BUFFER_WIDTH};

/// CP437 Box-Drawing glyph constants for text-mode window frames.
pub mod glyphs {
    // Single border
    pub const HORIZONTAL: u8 = 0xC4;      // ─
    pub const VERTICAL: u8 = 0xB3;        // │
    pub const TOP_LEFT: u8 = 0xDA;        // ┌
    pub const TOP_RIGHT: u8 = 0xBF;       // ┐
    pub const BOTTOM_LEFT: u8 = 0xC0;     // └
    pub const BOTTOM_RIGHT: u8 = 0xD9;    // ┘
    pub const T_DOWN: u8 = 0xC2;          // ┬
    pub const T_UP: u8 = 0xC1;            // ┴
    pub const T_RIGHT: u8 = 0xC3;         // ├
    pub const T_LEFT: u8 = 0xB4;          // ┤
    pub const CROSS: u8 = 0xC5;           // ┼

    // Double border
    pub const DBL_HORIZONTAL: u8 = 0xCD;  // ═
    pub const DBL_VERTICAL: u8 = 0xBA;    // ║
    pub const DBL_TOP_LEFT: u8 = 0xC9;    // ╔
    pub const DBL_TOP_RIGHT: u8 = 0xBB;   // ╗
    pub const DBL_BOTTOM_LEFT: u8 = 0xC8; // ╚
    pub const DBL_BOTTOM_RIGHT: u8 = 0xBC;// ╝
    pub const DBL_T_DOWN: u8 = 0xCB;      // ╦
    pub const DBL_T_UP: u8 = 0xCA;        // ╩
    pub const DBL_T_RIGHT: u8 = 0xCC;     // ╠
    pub const DBL_T_LEFT: u8 = 0xB9;      // ╣
    pub const DBL_CROSS: u8 = 0xCE;       // ╬

    // Symbols & Blocks
    pub const BLOCK_FULL: u8 = 0xDB;      // █
    pub const BLOCK_LOWER: u8 = 0xDC;     // ▄
    pub const BLOCK_UPPER: u8 = 0xDF;     // ▀
    pub const SQUARE: u8 = 0xFE;          // ■
    pub const ARROW_RIGHT: u8 = 0x10;     // ►
    pub const ARROW_LEFT: u8 = 0x11;      // ◄
    pub const MUSIC_NOTE: u8 = 0x0E;      // ♫
    pub const GEAR: u8 = 0x0F;            // ☼
}

/// A 4 KiB off-screen back buffer for double-buffered rendering.
#[derive(Clone, Copy)]
pub struct Compositor {
    pub cells: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

impl Compositor {
    /// Creates a new compositor initialized with blank lavender cells.
    pub const fn new() -> Self {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: ColorCode::new(Color::White, Color::Black),
        };
        Self {
            cells: [[blank; BUFFER_WIDTH]; BUFFER_HEIGHT],
        }
    }

    /// Clears the entire back buffer with the specified background and foreground color.
    pub fn clear(&mut self, fg: Color, bg: Color) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: ColorCode::new(fg, bg),
        };
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.cells[row][col] = blank;
            }
        }
    }

    /// Sets a single character cell at (col, row) if within screen bounds.
    pub fn put_char(&mut self, col: usize, row: usize, ch: u8, fg: Color, bg: Color) {
        if col < BUFFER_WIDTH && row < BUFFER_HEIGHT {
            self.cells[row][col] = ScreenChar {
                ascii_character: ch,
                color_code: ColorCode::new(fg, bg),
            };
        }
    }

    /// Draws an ASCII string horizontally starting at (col, row), clipped to `max_len` and screen width.
    pub fn draw_string(
        &mut self,
        col: usize,
        row: usize,
        s: &str,
        fg: Color,
        bg: Color,
    ) {
        if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
            return;
        }
        let mut curr_col = col;
        for b in s.bytes() {
            if curr_col >= BUFFER_WIDTH {
                break;
            }
            self.cells[row][curr_col] = ScreenChar {
                ascii_character: b,
                color_code: ColorCode::new(fg, bg),
            };
            curr_col += 1;
        }
    }

    /// Fills a rectangular region with a given character and colors.
    pub fn fill_rect(
        &mut self,
        start_col: usize,
        start_row: usize,
        width: usize,
        height: usize,
        ch: u8,
        fg: Color,
        bg: Color,
    ) {
        let max_col = (start_col + width).min(BUFFER_WIDTH);
        let max_row = (start_row + height).min(BUFFER_HEIGHT);
        let cell = ScreenChar {
            ascii_character: ch,
            color_code: ColorCode::new(fg, bg),
        };

        for row in start_row..max_row {
            for col in start_col..max_col {
                self.cells[row][col] = cell;
            }
        }
    }

    /// Draws a styled rectangular box with single or double CP437 borders.
    pub fn draw_box(
        &mut self,
        start_col: usize,
        start_row: usize,
        width: usize,
        height: usize,
        double_border: bool,
        fg: Color,
        bg: Color,
    ) {
        if width < 2 || height < 2 {
            return;
        }
        let end_col = (start_col + width - 1).min(BUFFER_WIDTH - 1);
        let end_row = (start_row + height - 1).min(BUFFER_HEIGHT - 1);

        let (tl, tr, bl, br, horiz, vert) = if double_border {
            (
                glyphs::DBL_TOP_LEFT,
                glyphs::DBL_TOP_RIGHT,
                glyphs::DBL_BOTTOM_LEFT,
                glyphs::DBL_BOTTOM_RIGHT,
                glyphs::DBL_HORIZONTAL,
                glyphs::DBL_VERTICAL,
            )
        } else {
            (
                glyphs::TOP_LEFT,
                glyphs::TOP_RIGHT,
                glyphs::BOTTOM_LEFT,
                glyphs::BOTTOM_RIGHT,
                glyphs::HORIZONTAL,
                glyphs::VERTICAL,
            )
        };

        // Corners
        self.put_char(start_col, start_row, tl, fg, bg);
        self.put_char(end_col, start_row, tr, fg, bg);
        self.put_char(start_col, end_row, bl, fg, bg);
        self.put_char(end_col, end_row, br, fg, bg);

        // Horizontal edges
        for col in (start_col + 1)..end_col {
            self.put_char(col, start_row, horiz, fg, bg);
            self.put_char(col, end_row, horiz, fg, bg);
        }

        // Vertical edges
        for row in (start_row + 1)..end_row {
            self.put_char(start_col, row, vert, fg, bg);
            self.put_char(end_col, row, vert, fg, bg);
        }
    }

    /// Blits the entire off-screen back buffer to physical MMIO VGA buffer at 0xb8000.
    ///
    /// Executes with interrupts disabled to guarantee atomic, tear-free frame presentation.
    /// When the graphical framebuffer is active, skips VGA 0xb8000 writes to prevent
    /// dual-plane contention and display flickering in VirtualBox.
    pub fn present(&self) {
        if crate::graphics::is_active() {
            return;
        }

        x86_64::instructions::interrupts::without_interrupts(|| {
            let vga_buffer = unsafe { &mut *(0xb8000 as *mut Buffer) };
            for row in 0..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    vga_buffer.chars[row][col].write(self.cells[row][col]);
                }
            }
        });
    }
}
