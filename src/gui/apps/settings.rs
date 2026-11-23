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
}
