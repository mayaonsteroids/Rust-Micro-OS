//! About Dialog Application for Desktop GUI.
//!
//! Presents verified OS identity, hardware architecture, theme details,
//! and strictly displays current user as "maya".

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;
use crate::kernel::identity;

pub struct AboutApp;

impl AboutApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders the About dialog into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // Inner Card Box (width 60, height 18)
        compositor.draw_box(cx + 2, cy + 1, 62, 19, false, Color::Cyan, Color::Black);

        // Header Title
        compositor.draw_string(cx + 18, cy + 2, "BUBBLEOS v0.1.0", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 12, cy + 3, "A little OS with big possibilities.", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 4, cy + 4, "--------------------------------------------------------", Color::LightGray, Color::Black);

        // System Details
        compositor.draw_string(cx + 6, cy + 5, "Operating System:  ", Color::White, Color::Black);
        compositor.draw_string(cx + 25, cy + 5, "BubbleOS v0.1.0 (Rust, no_std)", Color::LightCyan, Color::Black);

        compositor.draw_string(cx + 6, cy + 6, "Current User:      ", Color::White, Color::Black);
        compositor.draw_string(cx + 25, cy + 6, identity::get_username(), Color::LightGreen, Color::Black);

        compositor.draw_string(cx + 6, cy + 7, "System Hostname:   ", Color::White, Color::Black);
        identity::with_hostname(|h| {
            compositor.draw_string(cx + 25, cy + 7, h, Color::White, Color::Black);
        });

        compositor.draw_string(cx + 6, cy + 8, "Architecture:      x86_64 Long Mode (Ring 0 & Ring 3)", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 9, "Visual Palette:    Light Lavender (#E9DFFF Background)", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 10, "Kernel Heap Cap:   256 KiB (Linked-List Allocator)", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 11, "Network Subsystem: Intel 82540EM Gigabit (DMA RX/TX)", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 12, "Supported Network: ARP, IPv4, UDP, ICMP, DHCP, DNS, TCP", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 13, "Audio Hardware:    Intel ICH AC97 (00:05.0) + PC Speaker", Color::White, Color::Black);
        compositor.draw_string(cx + 6, cy + 14, "Console Driver:    VGA 80x25 MMIO (0xb8000) + UART COM1", Color::White, Color::Black);

        // Footer Guidance
        compositor.draw_string(cx + 4, cy + 16, "--------------------------------------------------------", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 8, cy + 17, "[Tab] / [1-8] Switch Windows | [Esc] Return to CLI", Color::LightCyan, Color::Black);
    }

    /// Renders the About dialog into the pixel-based window client area.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let ch = Window::CLIENT_PIXEL_HEIGHT;

        // Background
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 4, theme::WINDOW_SURFACE);

        // Center Information Card
        let card_w = 680;
        let card_h = 560;
        let card_x = cx + (cw.saturating_sub(card_w)) / 2;
        let card_y = cy + 40;

        draw::draw_shadow(fb, card_x, card_y, card_w, card_h, 8);
        draw::fill_rounded_rect(fb, card_x, card_y, card_w, card_h, 8, theme::WHITE);
        draw::draw_rounded_rect(fb, card_x, card_y, card_w, card_h, 8, theme::BORDER_LIGHT);

        // Top Banner inside Card
        draw::fill_rounded_rect(fb, card_x + 1, card_y + 1, card_w - 2, 80, 7, theme::DEEP_PURPLE);
        fb.fill_rect(card_x + 1, card_y + 50, card_w - 2, 32, theme::DEEP_PURPLE);

        // Draw 32x32 About Icon on Banner
        crate::graphics::image::draw_about_icon(fb, card_x + 28, card_y + 24);

        font::draw_string_scaled(fb, card_x + 72, card_y + 20, "BUBBLEOS", theme::WHITE, 2);
        font::draw_string(fb, card_x + 74, card_y + 54, "A little OS with big possibilities. - Release v0.1.0", Color::rgb(220, 205, 245), theme::DEEP_PURPLE);

        // Specification Rows
        let row_start_y = card_y + 104;
        let specs = [
            ("Operating System:", "BubbleOS v0.1.0 (Rust, no_std)", theme::PRIMARY_PURPLE),
            ("Short Description:", "A little OS with big possibilities.", theme::DARK_PLUM_TEXT),
            ("Current User:", identity::get_username(), Color::rgb(35, 140, 70)),
            ("System Hostname:", "micro-os", theme::DARK_PLUM_TEXT),
            ("Architecture:", "x86_64 Long Mode (Ring 0 Kernel / Ring 3 User Space)", theme::DARK_PLUM_TEXT),
            ("Display Controller:", "Bochs / VirtualBox BGA (1024x768 @ 32 bpp Truecolor)", theme::PRIMARY_PURPLE),
            ("Desktop Theme:", "Light Lavender (#E9DFFF Background, #38264F Plum Text)", theme::DARK_PLUM_TEXT),
            ("Kernel Memory Budget:", "256 KiB Linked-List Allocator (Zero Fragmentation)", theme::DARK_PLUM_TEXT),
            ("Audio Controller:", "Intel ICH AC97 (48 kHz Stereo DMA Ring) + PC Speaker", Color::rgb(35, 140, 70)),
            ("Network Subsystem:", "Intel 82540EM Gigabit (DHCP, DNS, TCP/IP, Wi-Fi Bridge)", Color::rgb(35, 140, 70)),
            ("Input Controller:", "Intel 8042 PS/2 Keyboard Controller (Scancode Set 1)", theme::DARK_PLUM_TEXT),
        ];

        for (i, (label, val, val_col)) in specs.iter().enumerate() {
            let row_y = row_start_y + i * 36;
            font::draw_string(fb, card_x + 32, row_y, label, Color::rgb(110, 95, 130), theme::WHITE);
            font::draw_string(fb, card_x + 210, row_y, val, *val_col, theme::WHITE);
            draw::draw_hline(fb, card_x + 32, card_x + card_w - 32, row_y + 24, Color::rgb(245, 240, 252));
        }

        // Bottom Card Guidance
        font::draw_string(fb, card_x + 80, card_y + card_h - 40, "[Tab] / [1-8] Switch Windows  |  [Esc] Minimize to Home Screen", Color::rgb(120, 105, 145), theme::WHITE);
    }
}
