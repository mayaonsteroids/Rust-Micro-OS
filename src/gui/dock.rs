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

    /// Returns the application title for logging and dispatch.
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

    /// Returns the long window title displayed in the window header bar.
    pub const fn window_title(self) -> &'static str {
        match self {
            AppId::Terminal => "Terminal",
            AppId::Network => "Network - Connection & Wi-Fi Control Center",
            AppId::Browser => "Web Browser",
            AppId::Files => "Files",
            AppId::Music => "Music Player",
            AppId::SysMon => "System Monitor",
            AppId::Settings => "Settings",
            AppId::About => "About",
        }
    }

    /// Returns the short dock label (centered below the icon).
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

    // 1. Fill dock background with Light Lavender (#E9DFFF)
    compositor.fill_rect(0, 0, dock_width, 25, b' ', Color::White, Color::Black);

    // 2. Draw vertical dividing border at column 11 in Deep Purple (#5C3C8A)
    for row in 0..25 {
        compositor.put_char(
            dock_width - 1,
            row,
            glyphs::DBL_VERTICAL,
            Color::Cyan,
            Color::Black,
        );
    }

    // 3. Render 8 application icons (each takes 3 rows: rows 0..23)
    for (i, app) in AppId::ALL.iter().enumerate() {
        let start_row = i * 3;
        let is_active = *app == active_app;

        let icon_col = 2; // columns 2..7 (width 6)

        let (border_fg, border_bg, text_fg, text_bg) = if is_active {
            // Active selection: elevated Light Window Surface (#F8F5FF) with Purple accent border
            (Color::LightCyan, Color::Black, Color::LightCyan, Color::LightGray)
        } else {
            // Normal unselected item: Dark Plum text on Light Lavender background
            (Color::DarkGray, Color::Black, Color::White, Color::Black)
        };

        // Selection Indicator at column 0: ►
        if is_active {
            compositor.put_char(0, start_row + 1, glyphs::ARROW_RIGHT, Color::LightCyan, Color::Black);
        } else {
            compositor.put_char(0, start_row + 1, b' ', Color::White, Color::Black);
        }

        // Row start_row: Top border of icon box
        compositor.put_char(icon_col, start_row, glyphs::TOP_LEFT, border_fg, border_bg);
        for c in 1..5 {
            compositor.put_char(icon_col + c, start_row, glyphs::HORIZONTAL, border_fg, border_bg);
        }
        compositor.put_char(icon_col + 5, start_row, glyphs::TOP_RIGHT, border_fg, border_bg);

        // Row start_row + 1: Box interior with distinctive icon glyphs
        compositor.put_char(icon_col, start_row + 1, glyphs::VERTICAL, border_fg, border_bg);

        match *app {
            AppId::Terminal => {
                // >_ terminal prompt
                compositor.draw_string(icon_col + 1, start_row + 1, " >_ ", text_fg, text_bg);
            }
            AppId::Network => {
                // [п] network adapter symbol (Greek pi 0xE3)
                compositor.put_char(icon_col + 1, start_row + 1, b' ', text_fg, text_bg);
                compositor.put_char(icon_col + 2, start_row + 1, b'[', text_fg, text_bg);
                compositor.put_char(icon_col + 3, start_row + 1, 0xE3, text_fg, text_bg); // CP437 Greek pi 'п'
                compositor.put_char(icon_col + 4, start_row + 1, b']', text_fg, text_bg);
            }
            AppId::Browser => {
                // WWW world wide web
                compositor.draw_string(icon_col + 1, start_row + 1, "WWW ", text_fg, text_bg);
            }
            AppId::Files => {
                // UFS Unix file system
                compositor.draw_string(icon_col + 1, start_row + 1, "UFS ", text_fg, text_bg);
            }
            AppId::Music => {
                // Spotify-inspired music note
                compositor.put_char(icon_col + 1, start_row + 1, b' ', text_fg, text_bg);
                compositor.put_char(icon_col + 2, start_row + 1, glyphs::MUSIC_NOTE, text_fg, text_bg);
                compositor.draw_string(icon_col + 3, start_row + 1, "  ", text_fg, text_bg);
            }
            AppId::SysMon => {
                // Hardware telemetry wave
                compositor.draw_string(icon_col + 1, start_row + 1, "-o--", text_fg, text_bg);
            }
            AppId::Settings => {
                // Configuration gear/asterisk
                compositor.put_char(icon_col + 1, start_row + 1, b' ', text_fg, text_bg);
                compositor.put_char(icon_col + 2, start_row + 1, glyphs::GEAR, text_fg, text_bg);
                compositor.draw_string(icon_col + 3, start_row + 1, "  ", text_fg, text_bg);
            }
            AppId::About => {
                // Information symbol
                compositor.draw_string(icon_col + 1, start_row + 1, " i  ", text_fg, text_bg);
            }
        }

        compositor.put_char(icon_col + 5, start_row + 1, glyphs::VERTICAL, border_fg, border_bg);

        // Row start_row + 2: Icon label centered in 11 columns
        let label = app.dock_label();
        let label_col = if label.len() < 10 {
            (10 - label.len()) / 2 + 1
        } else {
            1
        };
        let label_fg = if is_active { Color::LightCyan } else { Color::White };
        compositor.draw_string(label_col, start_row + 2, label, label_fg, Color::Black);
    }

    // Row 24: Navigation hints
    compositor.draw_string(0, 24, " [1-8/Tab] ", Color::LightCyan, Color::Black);
}

/// Renders the pixel-based Left Application Dock into the Framebuffer.
pub fn draw_pixel_dock(fb: &mut crate::graphics::Framebuffer, active_app: AppId) {
    use crate::graphics::{draw, font, theme, Color};
    use crate::graphics::image::AppIcon;

    let dock_w = 88;
    let dock_h = 768;

    // 1. Dock Background Surface (Soft Lavender #E6DEF6)
    let dock_bg = Color::rgb(230, 222, 246);
    fb.fill_rect(0, 0, dock_w, dock_h, dock_bg);

    // 2. Vertical Divider Line at right edge
    draw::draw_vline(fb, dock_w - 2, 0, dock_h, theme::BORDER_LIGHT);
    draw::draw_vline(fb, dock_w - 1, 0, dock_h, Color::rgb(245, 240, 255));

    // 3. Top System Emblem / Logo
    font::draw_string(fb, 24, 12, "MICRO", theme::PRIMARY_PURPLE, dock_bg);

    // 4. Render 8 Application Icons
    for (i, app) in AppId::ALL.iter().enumerate() {
        let is_selected = *app == active_app;
        let y_pos = 36 + i * 86; // 8 items evenly spaced in 768 height

        // If selected: Draw active pill indicator on left edge
        if is_selected {
            draw::fill_rounded_rect(fb, 2, y_pos + 6, 4, 40, 2, theme::PRIMARY_PURPLE);
        }

        // Draw the 32x32 genuine bitmap icon inside a 48x48 tile
        let icon_tile_x = 20;
        let icon_tile_y = y_pos;
        let icon = AppIcon::new(*app);
        icon.draw(fb, icon_tile_x, icon_tile_y, is_selected);

        // App Label centered below tile
        let label = app.dock_label();
        let text_w = label.len() * 8;
        let label_x = (dock_w.saturating_sub(text_w)) / 2;
        let label_fg = if is_selected { theme::PRIMARY_PURPLE } else { theme::DARK_PLUM_TEXT };
        font::draw_string(fb, label_x, y_pos + 56, label, label_fg, dock_bg);
    }

    // 5. Bottom Shortcut Hint
    font::draw_string(fb, 8, 740, "[1-8/Tab]", theme::DARK_PLUM_TEXT, dock_bg);
}
