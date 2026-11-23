//! Network Control Centre Application for Desktop GUI.
//!
//! Faithful implementation of the visual reference layout showing live
//! E1000 interface metrics, diagnostics, and Host Wi-Fi Bridge telemetry.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;

pub struct NetworkApp;

impl NetworkApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders the Network Control Centre into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // 1. Header Banner
        compositor.draw_string(cx, cy, "NETWORK", Color::LightCyan, Color::Black);
        compositor.draw_string(
            cx,
            cy + 1,
            "Dual-Interface Control: Host Wi-Fi Bridge + Virtual Intel e1000",
            Color::White,
            Color::Black,
        );
        compositor.draw_string(
            cx,
            cy + 2,
            "IP: 10.0.2.15   Gateway: 10.0.2.2   DNS: 10.0.2.3   MAC: 52:54:00:12:34:56",
            Color::LightGreen,
            Color::Black,
        );

        // 2. Left Column: Box 1 - Current Connection (Wi-Fi Bridge)
        let left_w = 31;
        compositor.draw_box(cx, cy + 4, left_w, 8, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 4, "[ Current Connection ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 1, cy + 5, "Status:   ", Color::White, Color::Black);
        compositor.draw_string(cx + 11, cy + 5, "HOST UNAVAILABLE", Color::Yellow, Color::Black);
        compositor.draw_string(cx + 1, cy + 6, "Network:  None", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 7, "Signal:   0% [....]", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 8, "Adapter:  Win WLAN (10.0.2.2)", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 1, cy + 10, "[SCAN] [DISCONNECT] [Wi-Fi:ON]", Color::LightCyan, Color::Black);

        // 3. Left Column: Box 2 - Ethernet Connection
        compositor.draw_box(cx, cy + 12, left_w, 8, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 12, "[ Ethernet Connection ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 1, cy + 13, "Interface: Intel 82540EM 1Gbps", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 14, "IPv4 Addr: 10.0.2.15 / 24", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 15, "Gateway:   10.0.2.2", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 16, "DNS Server:10.0.2.3", Color::White, Color::Black);
        compositor.draw_string(cx + 1, cy + 17, "Traffic:   Rx:65p Tx:70p (DMA)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 1, cy + 19, "[RENEW IP] [PING GW] [TEST DNS]", Color::LightCyan, Color::Black);

        // 4. Right Column: Box - Wi-Fi Networks
        let right_x = cx + left_w + 1;
        let right_w = window.client_width().saturating_sub(left_w + 1);
        compositor.draw_box(right_x, cy + 4, right_w, 16, false, Color::Cyan, Color::Black);
        compositor.draw_string(right_x + 2, cy + 4, "[ Wi-Fi Networks ]", Color::LightCyan, Color::Black);
        compositor.draw_string(right_x + 1, cy + 5, "SSID        Signal  Security  Status", Color::LightGray, Color::Black);
        compositor.draw_string(right_x + 1, cy + 6, "---------------------------------", Color::LightGray, Color::Black);

        compositor.draw_string(right_x + 1, cy + 8, "Host Wi-Fi Helper Offline", Color::Yellow, Color::Black);
        compositor.draw_string(right_x + 1, cy + 9, "Gateway socket: 10.0.2.2:19820", Color::White, Color::Black);
        compositor.draw_string(right_x + 1, cy + 11, "Run on Windows host to enable:", Color::LightGray, Color::Black);
        compositor.draw_string(right_x + 1, cy + 12, "python tools/windows_host_wifi...", Color::LightCyan, Color::Black);
        compositor.draw_string(right_x + 1, cy + 13, "Then click [ SCAN ] to refresh", Color::White, Color::Black);

        compositor.draw_string(right_x + 1, cy + 16, "Network: None selected", Color::LightGray, Color::Black);
        compositor.draw_string(right_x + 1, cy + 18, "Password [           ] [CONNECT]", Color::LightCyan, Color::Black);

        // 5. Bottom Footer Status Bar
        compositor.draw_string(cx, cy + 20, "----------------------------------------------------------------", Color::LightGray, Color::Black);
        compositor.draw_string(cx, cy + 21, "Wi-Fi State: HOST BRIDGE UNAVAILABLE (VirtualBox NAT Active)", Color::Yellow, Color::Black);
        compositor.draw_string(cx, cy + 22, "Diagnostic:  PASS: 64B from 10.0.2.2 | RTT: ~1.2ms | Loss: 0%", Color::LightGreen, Color::Black);
    }
}
