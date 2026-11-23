//! Window Primitives and Framing for Desktop GUI.
//!
//! Provides window borders, title bars, action buttons, and clipping bounds.

use crate::console::vga::Color;
use crate::gui::compositor::{glyphs, Compositor};

/// Represents a desktop window with position, size, and styling.
#[derive(Debug, Clone)]
pub struct Window {
    pub title: &'static str,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Window {
    /// Creates a standard desktop window filling the work area to the right of the dock.
    pub const fn standard(title: &'static str) -> Self {
        Self {
            title,
            x: 12,       // Immediately to the right of dock (0..11)
            y: 0,
            width: 68,   // Columns 12..79 (total 68 columns)
            height: 25,  // Rows 0..24
        }
    }

    /// Returns the client viewport X coordinate.
    pub const fn client_x(&self) -> usize {
        self.x + 1
    }

    /// Returns the client viewport Y coordinate.
    pub const fn client_y(&self) -> usize {
        self.y + 1
    }

    /// Returns the client viewport width.
    pub const fn client_width(&self) -> usize {
        if self.width > 2 {
            self.width - 2
        } else {
            0
        }
    }

    /// Returns the client viewport height.
    pub const fn client_height(&self) -> usize {
        if self.height > 2 {
            self.height - 2
        } else {
            0
        }
    }

    /// Clears the client area of the window with the default background.
    pub fn clear_client(&self, compositor: &mut Compositor) {
        compositor.fill_rect(
            self.client_x(),
            self.client_y(),
            self.client_width(),
            self.client_height(),
            b' ',
            Color::White,
            Color::Black,
        );
    }

    /// Draws the outer frame, title bar, and window buttons.
    pub fn draw_frame(&self, compositor: &mut Compositor, is_active: bool) {
        let border_fg = if is_active { Color::Cyan } else { Color::DarkGray };
        let title_bg = if is_active { Color::Blue } else { Color::DarkGray }; // Color::Blue is #7952A8 Purple
        let title_fg = Color::Pink; // #FFFFFF Pure White
        let win_bg = Color::Black;  // #E9DFFF Light Lavender

        // 1. Attractive Purple Title Bar across Row 0
        compositor.fill_rect(self.x, self.y, self.width, 1, b' ', title_fg, title_bg);

        // Window Title
        let title_str = self.title;
        let title_max_len = self.width.saturating_sub(14); // leave space for minimize/close buttons
        let mut title_buf = [0u8; 64];
        let mut t_len = 0;
        for b in title_str.bytes() {
            if t_len >= title_max_len || t_len >= title_buf.len() {
                break;
            }
            title_buf[t_len] = b;
            t_len += 1;
        }
        if let Ok(safe_title) = core::str::from_utf8(&title_buf[..t_len]) {
            compositor.draw_string(self.x + 2, self.y, safe_title, title_fg, title_bg);
        }

        // Window buttons at top right: "[-] [X]"
        let right_col = self.x + self.width - 1;
        if right_col >= 8 {
            let close_col = right_col - 3; // 3 chars for [X]
            let min_col = right_col - 7;   // 3 chars for [-]

            // Minimize [-] (Soft Rose #9B3356 on Purple)
            compositor.draw_string(min_col, self.y, "[-]", Color::Yellow, title_bg);

            // Close [X] (Readable Dark Red #8A1C14 on Purple)
            compositor.draw_string(close_col, self.y, "[X]", Color::LightRed, title_bg);
        }

        // 2. Left and Right Vertical Double Borders on Rows 1..23
        for row in (self.y + 1)..(self.y + self.height - 1) {
            compositor.put_char(self.x, row, glyphs::DBL_VERTICAL, border_fg, win_bg);
            compositor.put_char(self.x + self.width - 1, row, glyphs::DBL_VERTICAL, border_fg, win_bg);
        }

        // 3. Bottom Double Border on Row 24
        let bottom_row = self.y + self.height - 1;
        compositor.put_char(self.x, bottom_row, glyphs::DBL_BOTTOM_LEFT, border_fg, win_bg);
        for col in (self.x + 1)..(self.x + self.width - 1) {
            compositor.put_char(col, bottom_row, glyphs::DBL_HORIZONTAL, border_fg, win_bg);
        }
        compositor.put_char(self.x + self.width - 1, bottom_row, glyphs::DBL_BOTTOM_RIGHT, border_fg, win_bg);
    }
}
