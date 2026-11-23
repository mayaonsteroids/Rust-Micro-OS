//! 32×32 Pixel Application Icon Renderers.
//!
//! Provides genuine, hand-crafted, high-resolution 32×32 pixel bitmap icons
//! for all 8 Micro-OS desktop applications:
//! 1. Terminal
//! 2. Network Control Centre
//! 3. Single-Tab Web Browser
//! 4. Files Manager
//! 5. Spotify-Inspired Music Player
//! 6. System Monitor (SysMon)
//! 7. System Settings
//! 8. About Micro-OS

use crate::graphics::color::{theme, Color};
use crate::graphics::draw::{fill_rect, fill_rounded_rect, draw_rounded_rect, draw_hline, draw_vline};
use crate::graphics::framebuffer::Framebuffer;

pub const ICON_SIZE: usize = 32;

/// Application identifier enum for desktop icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIcon {
    Terminal,
    Network,
    Browser,
    Files,
    MusicPlayer,
    SysMon,
    Settings,
    About,
}

impl AppIcon {
    /// Constructs an `AppIcon` from an `AppId`.
    pub fn new(app_id: crate::gui::dock::AppId) -> Self {
        match app_id {
            crate::gui::dock::AppId::Terminal => AppIcon::Terminal,
            crate::gui::dock::AppId::Network => AppIcon::Network,
            crate::gui::dock::AppId::Browser => AppIcon::Browser,
            crate::gui::dock::AppId::Files => AppIcon::Files,
            crate::gui::dock::AppId::Music => AppIcon::MusicPlayer,
            crate::gui::dock::AppId::SysMon => AppIcon::SysMon,
            crate::gui::dock::AppId::Settings => AppIcon::Settings,
            crate::gui::dock::AppId::About => AppIcon::About,
        }
    }

    /// Renders the 32x32 application icon at `(x, y)` on the framebuffer.
    pub fn draw(&self, fb: &mut Framebuffer, x: usize, y: usize, is_selected: bool) {
        // Draw selected glow/halo if focused
        if is_selected {
            fill_rounded_rect(fb, x.saturating_sub(4), y.saturating_sub(4), ICON_SIZE + 8, ICON_SIZE + 8, 8, theme::DOCK_SELECTED);
            draw_rounded_rect(fb, x.saturating_sub(4), y.saturating_sub(4), ICON_SIZE + 8, ICON_SIZE + 8, 8, theme::PRIMARY_PURPLE);
        }

        match self {
            AppIcon::Terminal => draw_terminal_icon(fb, x, y),
            AppIcon::Network => draw_network_icon(fb, x, y),
            AppIcon::Browser => draw_browser_icon(fb, x, y),
            AppIcon::Files => draw_files_icon(fb, x, y),
            AppIcon::MusicPlayer => draw_music_icon(fb, x, y),
            AppIcon::SysMon => draw_sysmon_icon(fb, x, y),
            AppIcon::Settings => draw_settings_icon(fb, x, y),
            AppIcon::About => draw_about_icon(fb, x, y),
        }
    }
}

/// 1. Terminal Icon (32×32): Dark monitor tile with prompt `>_` and title dots.
fn draw_terminal_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x211333);
    let border = Color::from_hex(0x7952A8);
    let screen_bg = Color::from_hex(0x11081C);
    let prompt_green = Color::from_hex(0x2ECC71);
    let cursor_white = Color::from_hex(0xE9DFFF);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Title bar window dots
    fb.set_pixel(x + 5, y + 5, Color::from_hex(0xE74C3C));
    fb.set_pixel(x + 8, y + 5, Color::from_hex(0xF1C40F));
    fb.set_pixel(x + 11, y + 5, Color::from_hex(0x2ECC71));

    // Screen area
    fill_rect(fb, x + 4, y + 8, 24, 19, screen_bg);

    // Prompt `>`
    fb.set_pixel(x + 7, y + 12, prompt_green);
    fb.set_pixel(x + 8, y + 13, prompt_green);
    fb.set_pixel(x + 9, y + 14, prompt_green);
    fb.set_pixel(x + 10, y + 15, prompt_green);
    fb.set_pixel(x + 9, y + 16, prompt_green);
    fb.set_pixel(x + 8, y + 17, prompt_green);
    fb.set_pixel(x + 7, y + 18, prompt_green);

    // Cursor `_`
    draw_hline(fb, x + 13, y + 19, 6, cursor_white);
}

/// 2. Network Icon (32×32): Radiating Wi-Fi signal arcs & connected nodes.
fn draw_network_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x3B235E);
    let border = Color::from_hex(0x9B72CF);
    let signal_cyan = Color::from_hex(0x00E5FF);
    let signal_white = Color::from_hex(0xFFFFFF);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Outer Wi-Fi arc
    draw_hline(fb, x + 10, y + 7, 12, signal_cyan);
    fb.set_pixel(x + 8, y + 8, signal_cyan);
    fb.set_pixel(x + 9, y + 8, signal_cyan);
    fb.set_pixel(x + 22, y + 8, signal_cyan);
    fb.set_pixel(x + 23, y + 8, signal_cyan);
    fb.set_pixel(x + 6, y + 9, signal_cyan);
    fb.set_pixel(x + 7, y + 9, signal_cyan);
    fb.set_pixel(x + 24, y + 9, signal_cyan);
    fb.set_pixel(x + 25, y + 9, signal_cyan);

    // Middle Wi-Fi arc
    draw_hline(fb, x + 12, y + 12, 8, signal_white);
    fb.set_pixel(x + 10, y + 13, signal_white);
    fb.set_pixel(x + 11, y + 13, signal_white);
    fb.set_pixel(x + 20, y + 13, signal_white);
    fb.set_pixel(x + 21, y + 13, signal_white);
    fb.set_pixel(x + 9, y + 14, signal_white);
    fb.set_pixel(x + 22, y + 14, signal_white);

    // Inner Wi-Fi arc
    draw_hline(fb, x + 14, y + 17, 4, signal_cyan);
    fb.set_pixel(x + 13, y + 18, signal_cyan);
    fb.set_pixel(x + 18, y + 18, signal_cyan);

    // Center base broadcast dot (2x2)
    fill_rect(fb, x + 15, y + 21, 2, 2, signal_white);

    // Ethernet node base line
    draw_hline(fb, x + 8, y + 26, 16, Color::from_hex(0x7952A8));
    fill_rect(fb, x + 7, y + 25, 3, 3, signal_cyan);
    fill_rect(fb, x + 22, y + 25, 3, 3, signal_cyan);
}

/// 3. Browser Icon (32×32): World globe with meridians and equator.
fn draw_browser_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x1F2A50);
    let border = Color::from_hex(0x4A69BD);
    let globe_blue = Color::from_hex(0x2980B9);
    let land_cyan = Color::from_hex(0x1ABC9C);
    let ring_white = Color::from_hex(0xECF0F1);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Globe circle fill
    fill_rounded_rect(fb, x + 5, y + 5, 22, 22, 10, globe_blue);
    draw_rounded_rect(fb, x + 5, y + 5, 22, 22, 10, ring_white);

    // Equator line
    draw_hline(fb, x + 6, y + 16, 20, ring_white);

    // Vertical Meridian Ellipse
    draw_vline(fb, x + 16, y + 6, 20, ring_white);
    fb.set_pixel(x + 13, y + 9, land_cyan);
    fb.set_pixel(x + 12, y + 10, land_cyan);
    fb.set_pixel(x + 11, y + 13, land_cyan);
    fb.set_pixel(x + 11, y + 19, land_cyan);
    fb.set_pixel(x + 12, y + 22, land_cyan);
    fb.set_pixel(x + 13, y + 23, land_cyan);

    fb.set_pixel(x + 19, y + 9, land_cyan);
    fb.set_pixel(x + 20, y + 10, land_cyan);
    fb.set_pixel(x + 21, y + 13, land_cyan);
    fb.set_pixel(x + 21, y + 19, land_cyan);
    fb.set_pixel(x + 20, y + 22, land_cyan);
    fb.set_pixel(x + 19, y + 23, land_cyan);

    // Compass Pointer in corner
    fb.set_pixel(x + 22, y + 8, Color::from_hex(0xE74C3C));
    fb.set_pixel(x + 23, y + 7, Color::from_hex(0xE74C3C));
}

/// 4. Files Icon (32×32): Warm folder with tab and paper insert.
fn draw_files_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let folder_dark = Color::from_hex(0xD35400);
    let folder_light = Color::from_hex(0xF39C12);
    let paper_white = Color::from_hex(0xFFFFFF);
    let line_purple = Color::from_hex(0x7952A8);

    // Folder Back Tab
    fill_rect(fb, x + 4, y + 6, 11, 6, folder_dark);
    fill_rect(fb, x + 4, y + 9, 24, 18, folder_dark);

    // Document Paper inside folder
    fill_rect(fb, x + 8, y + 5, 16, 16, paper_white);
    draw_hline(fb, x + 10, y + 8, 12, line_purple);
    draw_hline(fb, x + 10, y + 11, 10, line_purple);
    draw_hline(fb, x + 10, y + 14, 12, line_purple);

    // Folder Front Flap
    fill_rounded_rect(fb, x + 3, y + 12, 26, 15, 3, folder_light);
    draw_rounded_rect(fb, x + 3, y + 12, 26, 15, 3, Color::from_hex(0xE67E22));
}

/// 5. Music Player Icon (32×32): Spotify-inspired badge with white musical note.
fn draw_music_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x1DB954); // Spotify Green
    let note_white = Color::from_hex(0xFFFFFF);

    // Circular green badge
    fill_rounded_rect(fb, x + 2, y + 2, 28, 28, 14, base_bg);
    draw_rounded_rect(fb, x + 2, y + 2, 28, 28, 14, Color::from_hex(0x179843));

    // Musical Eighth Note stem and flag
    // Left Note Head
    fill_rounded_rect(fb, x + 8, y + 19, 6, 5, 2, note_white);
    // Right Note Head
    fill_rounded_rect(fb, x + 18, y + 16, 6, 5, 2, note_white);

    // Left Stem
    fill_rect(fb, x + 12, y + 9, 2, 12, note_white);
    // Right Stem
    fill_rect(fb, x + 22, y + 6, 2, 12, note_white);

    // Connecting Top Beam
    fill_rect(fb, x + 12, y + 6, 12, 4, note_white);
}

/// 6. System Monitor Icon (32×32): Heartbeat pulse / CPU activity graph.
fn draw_sysmon_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x181824);
    let border = Color::from_hex(0x38264F);
    let grid_dim = Color::from_hex(0x282438);
    let pulse_green = Color::from_hex(0x00FF88);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Grid background
    draw_hline(fb, x + 4, y + 10, 24, grid_dim);
    draw_hline(fb, x + 4, y + 16, 24, grid_dim);
    draw_hline(fb, x + 4, y + 22, 24, grid_dim);
    draw_vline(fb, x + 10, y + 4, 24, grid_dim);
    draw_vline(fb, x + 16, y + 4, 24, grid_dim);
    draw_vline(fb, x + 22, y + 4, 24, grid_dim);

    // ECG Heartbeat wave
    draw_hline(fb, x + 4, y + 16, 5, pulse_green);
    fb.set_pixel(x + 9, y + 15, pulse_green);
    fb.set_pixel(x + 10, y + 13, pulse_green);
    fb.set_pixel(x + 11, y + 10, pulse_green); // Peak up
    fb.set_pixel(x + 12, y + 7, pulse_green);
    fb.set_pixel(x + 13, y + 11, pulse_green);
    fb.set_pixel(x + 14, y + 16, pulse_green);
    fb.set_pixel(x + 15, y + 21, pulse_green); // Peak down
    fb.set_pixel(x + 16, y + 24, pulse_green);
    fb.set_pixel(x + 17, y + 20, pulse_green);
    fb.set_pixel(x + 18, y + 16, pulse_green);
    draw_hline(fb, x + 19, y + 16, 9, pulse_green);
}

/// 7. Settings Icon (32×32): Mechanical gear cog.
fn draw_settings_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x2C3A47);
    let border = Color::from_hex(0x596275);
    let gear_gray = Color::from_hex(0xBDC3C7);
    let gear_dark = Color::from_hex(0x7F8C8D);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Central circular body
    fill_rounded_rect(fb, x + 9, y + 9, 14, 14, 7, gear_gray);

    // 4 Cardinal Teeth
    fill_rect(fb, x + 13, y + 5, 6, 5, gear_gray);  // Top
    fill_rect(fb, x + 13, y + 22, 6, 5, gear_gray); // Bottom
    fill_rect(fb, x + 5, y + 13, 5, 6, gear_gray);  // Left
    fill_rect(fb, x + 22, y + 13, 5, 6, gear_gray); // Right

    // 4 Diagonal Teeth
    fill_rect(fb, x + 7, y + 7, 4, 4, gear_gray);
    fill_rect(fb, x + 21, y + 7, 4, 4, gear_gray);
    fill_rect(fb, x + 7, y + 21, 4, 4, gear_gray);
    fill_rect(fb, x + 21, y + 21, 4, 4, gear_gray);

    // Center Hole
    fill_rounded_rect(fb, x + 13, y + 13, 6, 6, 3, base_bg);
    draw_rounded_rect(fb, x + 13, y + 13, 6, 6, 3, gear_dark);
}

/// 8. About Icon (32×32): Royal purple badge with information `i` symbol.
pub fn draw_about_icon(fb: &mut Framebuffer, x: usize, y: usize) {
    let base_bg = Color::from_hex(0x5C3C8A); // Deep Theme Purple
    let border = Color::from_hex(0x9B72CF);
    let gold_crown = Color::from_hex(0xF1C40F);
    let info_white = Color::from_hex(0xFFFFFF);

    fill_rounded_rect(fb, x, y, 32, 32, 6, base_bg);
    draw_rounded_rect(fb, x, y, 32, 32, 6, border);

    // Crown at top (Gold)
    fb.set_pixel(x + 11, y + 7, gold_crown);
    fb.set_pixel(x + 16, y + 6, gold_crown);
    fb.set_pixel(x + 21, y + 7, gold_crown);
    draw_hline(fb, x + 11, y + 9, 11, gold_crown);

    // Information Dot
    fill_rect(fb, x + 14, y + 12, 4, 4, info_white);

    // Information Stem & Serif
    draw_hline(fb, x + 13, y + 18, 4, info_white);
    fill_rect(fb, x + 15, y + 18, 3, 7, info_white);
    draw_hline(fb, x + 13, y + 24, 7, info_white);
}
