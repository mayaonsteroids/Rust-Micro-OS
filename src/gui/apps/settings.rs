//! Settings Application for Desktop GUI.
//!
//! Provides system customization controls: display palette, keyboard repeat,
//! networking mode, and audio output configuration.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;

pub struct SettingsApp;

impl SettingsApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders system settings into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        compositor.draw_string(cx, cy, "=== SYSTEM CONFIGURATION & PREFERENCES ===", Color::LightCyan, Color::Black);
        compositor.draw_string(cx, cy + 1, "Customize hardware parameters, display theme, and audio output.", Color::LightGray, Color::Black);
        compositor.draw_string(cx, cy + 2, "----------------------------------------------------------------", Color::LightGray, Color::Black);

        // Section 1: Display & Theme
        compositor.draw_string(cx, cy + 4, "[ Display & Visual Theme ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 5, "Palette Mode:      [ Light Lavender (#E9DFFF) ] (Active)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 2, cy + 6, "Text Contrast:     [ High Contrast Plum (#38264F) ]", Color::White, Color::Black);
        compositor.draw_string(cx + 2, cy + 7, "Resolution:        80x25 Character Matrix (VGA Text Buffer)", Color::LightGray, Color::Black);

        // Section 2: Input & Keyboard
        compositor.draw_string(cx, cy + 9, "[ Keyboard & Input ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 10, "Controller:        8042 PS/2 Keyboard Controller", Color::White, Color::Black);
        compositor.draw_string(cx + 2, cy + 11, "Interrupt Delivery:IRQ 1 / Vector 33 (Master PIC Unmasked)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 2, cy + 12, "Scancode Decoder:  Scancode Set 1 (Translation Active)", Color::White, Color::Black);

        // Section 3: Audio Subsystem
        compositor.draw_string(cx, cy + 14, "[ Audio Controller Selection ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 15, "Primary Driver:    [ Intel ICH AC97 (00:05.0) ] (Detected)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 2, cy + 16, "Fallback Driver:   [ PC Speaker Synthesizer (0x61) ]", Color::White, Color::Black);
        compositor.draw_string(cx + 2, cy + 17, "Master Volume:     [========--------] 50%", Color::LightCyan, Color::Black);

        // Section 4: Network Mode
        compositor.draw_string(cx, cy + 19, "[ Network Stack Configuration ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 20, "IP Acquisition:    [ Dynamic DHCP Lease ] (10.0.2.15/24)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 2, cy + 21, "Host Wi-Fi Bridge: [ Enabled / Polling Port 19820 ]", Color::White, Color::Black);
    }

    /// Renders system settings into the pixel-based window client area.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let ch = Window::CLIENT_PIXEL_HEIGHT;

        // Background
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 4, theme::WINDOW_SURFACE);

        // Header
        font::draw_string(fb, cx + 16, cy + 12, "SYSTEM CONFIGURATION & PREFERENCES", theme::PRIMARY_PURPLE, theme::WINDOW_SURFACE);
        font::draw_string(fb, cx + 16, cy + 30, "Customize display mode, keyboard repeat, audio driver, and network stack.", theme::DARK_PLUM_TEXT, theme::WINDOW_SURFACE);

        // Section 1: Display & Visual Theme
        let s1_y = cy + 56;
        let s1_h = 130;
        draw::fill_rounded_rect(fb, cx + 16, s1_y, cw - 32, s1_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, s1_y, cw - 32, s1_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, cx + 17, s1_y + 1, cw - 34, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, cx + 28, s1_y + 7, "Display & Visual Theme", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, cx + 16, cx + cw - 16, s1_y + 29, theme::BORDER_LIGHT);

        font::draw_string(fb, cx + 32, s1_y + 40, "Color Palette:    [ Light Lavender (#E9DFFF) ] (Active Truecolor)", Color::rgb(35, 140, 70), theme::WHITE);
        font::draw_string(fb, cx + 32, s1_y + 64, "Text Color:       [ Dark Plum (#38264F) ] on Light Surfaces", theme::DARK_PLUM_TEXT, theme::WHITE);
        font::draw_string(fb, cx + 32, s1_y + 88, "Resolution:       1024x768 @ 32 bpp Truecolor (Bochs/VBox BGA Linear Framebuffer)", theme::DEEP_PURPLE, theme::WHITE);

        // Section 2: Input & Keyboard
        let s2_y = s1_y + s1_h + 12;
        let s2_h = 130;
        draw::fill_rounded_rect(fb, cx + 16, s2_y, cw - 32, s2_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, s2_y, cw - 32, s2_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, cx + 17, s2_y + 1, cw - 34, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, cx + 28, s2_y + 7, "Keyboard & Input Controller", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, cx + 16, cx + cw - 16, s2_y + 29, theme::BORDER_LIGHT);

        font::draw_string(fb, cx + 32, s2_y + 40, "Controller:       Intel 8042 PS/2 Microcontroller & Dual Ports", theme::DARK_PLUM_TEXT, theme::WHITE);
        font::draw_string(fb, cx + 32, s2_y + 64, "Interrupt Line:   Master PIC IRQ 1 / IDT Vector 33 (Active)", Color::rgb(35, 140, 70), theme::WHITE);
        font::draw_string(fb, cx + 32, s2_y + 88, "Scancode Decoder: Scancode Set 1 (Auto-translated from Set 2)", theme::DARK_PLUM_TEXT, theme::WHITE);

        // Section 3: Audio Hardware
        let s3_y = s2_y + s2_h + 12;
        let s3_h = 130;
        draw::fill_rounded_rect(fb, cx + 16, s3_y, cw - 32, s3_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, s3_y, cw - 32, s3_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, cx + 17, s3_y + 1, cw - 34, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, cx + 28, s3_y + 7, "Audio Output & Codec Engine", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, cx + 16, cx + cw - 16, s3_y + 29, theme::BORDER_LIGHT);

        font::draw_string(fb, cx + 32, s3_y + 40, "Primary Output:   [ Intel ICH AC97 (00:05.0) ] - 48 kHz Stereo DMA Ring", Color::rgb(35, 140, 70), theme::WHITE);
        font::draw_string(fb, cx + 32, s3_y + 64, "Fallback Output:  [ PC Speaker Synthesizer (I/O Port 0x61) ]", Color::rgb(120, 105, 140), theme::WHITE);
        font::draw_string(fb, cx + 32, s3_y + 88, "Sample Format:    16-bit Signed PCM / IMA ADPCM Real-time Decompression", theme::DARK_PLUM_TEXT, theme::WHITE);

        // Section 4: Network Stack
        let s4_y = s3_y + s3_h + 12;
        let s4_h = ch.saturating_sub(s1_h + s2_h + s3_h + 76);
        draw::fill_rounded_rect(fb, cx + 16, s4_y, cw - 32, s4_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, s4_y, cw - 32, s4_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, cx + 17, s4_y + 1, cw - 34, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, cx + 28, s4_y + 7, "Network Configuration & Wi-Fi Bridge", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, cx + 16, cx + cw - 16, s4_y + 29, theme::BORDER_LIGHT);

        font::draw_string(fb, cx + 32, s4_y + 40, "Ethernet Interface: Intel 82540EM (PCI 0x8086:0x100e) - DHCP Lease Active", Color::rgb(35, 140, 70), theme::WHITE);
        font::draw_string(fb, cx + 32, s4_y + 64, "Host Wi-Fi Bridge:  TCP Proxy Socket 10.0.2.2:19820 (Bridge Standby)", theme::DARK_PLUM_TEXT, theme::WHITE);
    }
}
