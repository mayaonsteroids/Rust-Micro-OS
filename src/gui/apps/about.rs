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
        compositor.draw_string(cx + 16, cy + 2, "RUST MICRO-OS v0.1.0", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 13, cy + 3, "Desktop Edition - Phase 1 Milestone", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 4, cy + 4, "--------------------------------------------------------", Color::LightGray, Color::Black);

        // System Details
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
}
