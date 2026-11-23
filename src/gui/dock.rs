//! Left-Hand Vertical Application Dock for Desktop GUI.
//!
//! Renders the 8 application icons inspired by the reference screenshot,
//! handling icon selection, highlighting, and application switching.

use crate::console::vga::Color;
use crate::gui::compositor::{glyphs, Compositor};

/// The 8 primary desktop applications accessible via the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppId {
    Terminal = 0,
    Network = 1,
    Browser = 2,
    Files = 3,
    Music = 4,
    SysMon = 5,
    Settings = 6,
    About = 7,
}

impl AppId {
    pub const ALL: [AppId; 8] = [
        AppId::Terminal,
        AppId::Network,
        AppId::Browser,
        AppId::Files,
        AppId::Music,
        AppId::SysMon,
        AppId::Settings,
        AppId::About,
    ];

    /// Returns the application title.
    pub const fn title(self) -> &'static str {
        match self {
            AppId::Terminal => "Terminal",
            AppId::Network => "Network",
            AppId::Browser => "Browser",
            AppId::Files => "Files",
            AppId::Music => "Music Player",
            AppId::SysMon => "System Monitor",
            AppId::Settings => "Settings",
            AppId::About => "About",
        }
    }

    /// Returns the short dock label (max 8 characters).
    pub const fn dock_label(self) -> &'static str {
        match self {
            AppId::Terminal => "Terminal",
            AppId::Network => "Network",
            AppId::Browser => "Browser",
            AppId::Files => "Files",
            AppId::Music => "Music",
            AppId::SysMon => "SysMon",
            AppId::Settings => "Settings",
            AppId::About => "About",
        }
    }

    /// Returns the 4-character icon glyph representation.
    pub const fn icon_symbol(self) -> &'static str {
        match self {
            AppId::Terminal => " >_ ",
            AppId::Network => " ## ",
            AppId::Browser => " WWW",
            AppId::Files => " UFS",
            AppId::Music => "  * ", // Musical note or star
            AppId::SysMon => " ~~~",
            AppId::Settings => " [o]",
            AppId::About => " (i)",
        }
    }

    /// Returns the next app in sequential order.
    pub fn next(self) -> Self {
        let idx = (self as u8 + 1) % 8;
        Self::from_index(idx)
    }

    /// Returns the previous app in sequential order.
    pub fn prev(self) -> Self {
        let idx = if (self as u8) == 0 { 7 } else { self as u8 - 1 };
        Self::from_index(idx)
    }

    /// Maps a 0..7 index to `AppId`.
    pub const fn from_index(idx: u8) -> Self {
        match idx % 8 {
            0 => AppId::Terminal,
            1 => AppId::Network,
            2 => AppId::Browser,
            3 => AppId::Files,
            4 => AppId::Music,
            5 => AppId::SysMon,
            6 => AppId::Settings,
            _ => AppId::About,
        }
    }
}

/// Renders the left dock into the compositor back buffer.
pub fn draw_dock(compositor: &mut Compositor, active_app: AppId) {
    let dock_width = 12; // columns 0..11

    // 1. Fill dock background
    compositor.fill_rect(0, 0, dock_width, 25, b' ', Color::White, Color::Black);

    // 2. Draw vertical dividing border at column 11
    for row in 0..25 {
        compositor.put_char(
            dock_width - 1,
            row,
            glyphs::DBL_VERTICAL,
            Color::Cyan,
            Color::Black,
        );
    }

    // 3. Render 8 application icons (each takes 3 rows: row 0..23)
    for (i, app) in AppId::ALL.iter().enumerate() {
        let start_row = i * 3;
        let is_active = *app == active_app;

        let (box_fg, box_bg, text_fg, text_bg) = if is_active {
            // Highlighted active item (Medium Purple background or vibrant text)
            (Color::LightCyan, Color::Black, Color::LightCyan, Color::Black)
        } else {
            // Normal muted item
            (Color::LightGray, Color::Black, Color::White, Color::Black)
        };

        // Icon Box (Row 0 & 1): e.g. [ >_ ]
        let icon_col = 2;
        if is_active {
            // Draw active indicator arrow at column 0
            compositor.put_char(0, start_row + 1, glyphs::ARROW_RIGHT, Color::LightCyan, Color::Black);
        } else {
            compositor.put_char(0, start_row + 1, b' ', Color::White, Color::Black);
        }

        // Box border top
        compositor.draw_string(icon_col, start_row, "+----+", box_fg, box_bg);

        // Box interior with icon glyph
        compositor.put_char(icon_col, start_row + 1, b'|', box_fg, box_bg);
        if *app == AppId::Music {
            // Render custom music note CP437 glyph
            compositor.draw_string(icon_col + 1, start_row + 1, " ", text_fg, text_bg);
            compositor.put_char(icon_col + 2, start_row + 1, glyphs::MUSIC_NOTE, text_fg, text_bg);
            compositor.draw_string(icon_col + 3, start_row + 1, "  ", text_fg, text_bg);
        } else {
            compositor.draw_string(icon_col + 1, start_row + 1, app.icon_symbol(), text_fg, text_bg);
        }
        compositor.put_char(icon_col + 5, start_row + 1, b'|', box_fg, box_bg);

        // Icon label on row 2 (centered in 11 columns)
        let label = app.dock_label();
        let label_col = if label.len() < 10 {
            (10 - label.len()) / 2 + 1
        } else {
            1
        };
        compositor.draw_string(label_col, start_row + 2, label, text_fg, text_bg);
    }

    // Row 24: Navigation hints
    compositor.draw_string(0, 24, " [1-8/Tab] ", Color::LightCyan, Color::Black);
}
