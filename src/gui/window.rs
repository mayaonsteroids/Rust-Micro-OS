//! Window Primitives and Framing for Desktop GUI.
//!
//! Provides window borders, title bars, action buttons, and clipping bounds.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;

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
        let border_fg = if is_active { Color::Cyan } else { Color::LightGray };
        let title_fg = if is_active { Color::LightCyan } else { Color::White };
        let bg = Color::Black;

        // Draw double outer box
        compositor.draw_box(
            self.x,
            self.y,
            self.width,
            self.height,
            true, // double border
            border_fg,
            bg,
        );

        // Title bar header: e.g. " [ Terminal ] "
        let title_str = self.title;
        let title_start = self.x + 3;
        compositor.put_char(title_start - 1, self.y, b'[', border_fg, bg);
        compositor.draw_string(title_start, self.y, title_str, title_fg, bg);
        compositor.put_char(title_start + title_str.len(), self.y, b']', border_fg, bg);

        // Window buttons at top right: "[-] [X]"
        let right_col = self.x + self.width - 1;
        if right_col >= 8 {
            let close_col = right_col - 4;
            let min_col = right_col - 8;

            // Minimize [-]
            compositor.draw_string(min_col, self.y, "[-]", Color::Yellow, bg);

            // Close [X]
            compositor.draw_string(close_col, self.y, "[X]", Color::LightRed, bg);
        }
    }
}
