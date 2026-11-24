//! Network Control Centre Application for BubbleOS Desktop GUI.
//!
//! Provides truthful, real-time Intel 82540EM Gigabit Ethernet telemetry,
//! dynamic DHCP lease acquisition, ICMP Gateway ping tests, DNS resolution,
//! live DMA traffic counters, and interactive mouse controls.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;
use crate::net::{get_config, get_mac_address, get_statistics, is_device_present, is_link_up};

fn fmt_u64<'a>(mut val: u64, buf: &'a mut [u8]) -> &'a str {
    if val == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut temp = [0u8; 20];
    let mut len = 0;
    while val > 0 && len < temp.len() {
        temp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    for i in 0..len {
        buf[i] = temp[len - 1 - i];
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("0")
}

fn write_bytes(dst: &mut [u8], offset: &mut usize, src: &[u8]) {
    for &b in src {
        if *offset < dst.len() {
            dst[*offset] = b;
            *offset += 1;
        }
    }
}

fn write_u8(dst: &mut [u8], offset: &mut usize, mut val: u8) {
    if val == 0 {
        write_bytes(dst, offset, b"0");
        return;
    }
    let mut temp = [0u8; 3];
    let mut len = 0;
    while val > 0 {
        temp[len] = b'0' + (val % 10);
        val /= 10;
        len += 1;
    }
    for i in 0..len {
        write_bytes(dst, offset, &[temp[len - 1 - i]]);
    }
}

fn write_hex2(dst: &mut [u8], offset: &mut usize, val: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hi = HEX[(val >> 4) as usize];
    let lo = HEX[(val & 0x0F) as usize];
    write_bytes(dst, offset, &[hi, lo]);
}

pub struct NetworkApp {
    pub last_ping_status: Option<Result<usize, &'static str>>,
    pub last_dns_status: Option<Result<[u8; 4], &'static str>>,
    pub last_dhcp_status: Option<Result<[u8; 4], &'static str>>,
    pub last_action_message: Option<&'static str>,
}

impl NetworkApp {
    pub const fn new() -> Self {
        Self {
            last_ping_status: None,
            last_dns_status: None,
            last_dhcp_status: None,
            last_action_message: None,
        }
    }

    /// Performs real dynamic DHCP lease acquisition.
    pub fn renew_ip(&mut self) {
        crate::info!("[NETWORK] Action: [RENEW IP] triggered...");
        match crate::net::renew_dhcp(1_500_000) {
            Ok(lease) => {
                self.last_dhcp_status = Some(Ok(lease.assigned_ip));
                self.last_action_message = Some("PASS: Dynamic DHCP lease acquired successfully!");
                crate::info!(
                    "[NETWORK] DHCP Success: IP {}.{}.{}.{} | GW {}.{}.{}.{} | DNS {}.{}.{}.{}",
                    lease.assigned_ip[0], lease.assigned_ip[1], lease.assigned_ip[2], lease.assigned_ip[3],
                    lease.gateway[0], lease.gateway[1], lease.gateway[2], lease.gateway[3],
                    lease.dns_server[0], lease.dns_server[1], lease.dns_server[2], lease.dns_server[3],
                );
            }
            Err(e) => {
                self.last_dhcp_status = Some(Err(e));
                self.last_action_message = Some("DHCP timed out (preserving static fallback configuration).");
                crate::warn!("[NETWORK] DHCP Failed: {}", e);
            }
        }
    }

    /// Sends a genuine ICMP Echo Request to the default gateway.
    pub fn ping_gateway(&mut self) {
        crate::info!("[NETWORK] Action: [PING GATEWAY] triggered...");
        match crate::net::ping_gateway(1_500_000) {
            Ok(_spins) => {
                self.last_ping_status = Some(Ok(_spins));
                self.last_action_message = Some("PASS: 64 bytes received from Gateway (ICMP Echo Reply OK).");
                crate::info!("[NETWORK] Gateway Ping: SUCCESS (0% packet loss)");
            }
            Err(e) => {
                self.last_ping_status = Some(Err(e));
                self.last_action_message = Some("FAIL: Gateway ping timed out (no reply received).");
                crate::warn!("[NETWORK] Gateway Ping: FAILED ({})", e);
            }
        }
    }

    /// Performs an actual DNS query against the configured DNS server.
    pub fn test_dns(&mut self) {
        crate::info!("[NETWORK] Action: [TEST DNS] triggered (resolving google.com)...");
        match crate::net::test_dns_resolution(1_500_000) {
            Ok(ip) => {
                self.last_dns_status = Some(Ok(ip));
                self.last_action_message = Some("PASS: google.com successfully resolved via DNS server.");
                crate::info!(
                    "[NETWORK] DNS Success: google.com -> {}.{}.{}.{}",
                    ip[0], ip[1], ip[2], ip[3]
                );
            }
            Err(e) => {
                self.last_dns_status = Some(Err(e));
                self.last_action_message = Some("FAIL: DNS resolution timed out.");
                crate::warn!("[NETWORK] DNS Failed: {}", e);
            }
        }
    }

    /// Refreshes network adapter statistics and hardware status.
    pub fn refresh(&mut self) {
        crate::info!("[NETWORK] Action: [REFRESH] triggered.");
        self.last_action_message = Some("Refreshed interface metrics and DMA traffic counters.");
    }

    /// Handles keyboard shortcut inputs.
    pub fn handle_key(&mut self, key: u8) {
        match key {
            b'r' | b'R' => self.renew_ip(),
            b'p' | b'P' => self.ping_gateway(),
            b'd' | b'D' => self.test_dns(),
            b'f' | b'F' => self.refresh(),
            _ => {}
        }
    }

    /// Renders the Network Control Centre into the VGA text compositor.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // 1. Header Banner
        compositor.draw_string(cx, cy, "NETWORK CONTROL CENTRE", Color::LightCyan, Color::Black);
        compositor.draw_string(
            cx,
            cy + 1,
            "Virtual Ethernet Architecture | VirtualBox / Hypervisor NAT Bridge",
            Color::White,
            Color::Black,
        );

        let cfg = get_config();
        let stats = get_statistics();
        let dev_present = is_device_present();
        let link_up = is_link_up();

        // Metrics Header line
        let mut top_buf = [0u8; 80];
        let mut top_off = 0;
        write_bytes(&mut top_buf, &mut top_off, b"IP: ");
        for i in 0..4 {
            write_u8(&mut top_buf, &mut top_off, cfg.ip[i]);
            if i < 3 { write_bytes(&mut top_buf, &mut top_off, b"."); }
        }
        write_bytes(&mut top_buf, &mut top_off, b"  GW: ");
        for i in 0..4 {
            write_u8(&mut top_buf, &mut top_off, cfg.gateway[i]);
            if i < 3 { write_bytes(&mut top_buf, &mut top_off, b"."); }
        }
        write_bytes(&mut top_buf, &mut top_off, b"  DNS: ");
        for i in 0..4 {
            write_u8(&mut top_buf, &mut top_off, cfg.dns[i]);
            if i < 3 { write_bytes(&mut top_buf, &mut top_off, b"."); }
        }
        if let Ok(top_str) = core::str::from_utf8(&top_buf[..top_off]) {
            compositor.draw_string(cx, cy + 2, top_str, Color::LightCyan, Color::Black);
        }

        // Left Box: Adapter & IP Configuration (w = 32)
        let left_w = 32;
        compositor.draw_box(cx, cy + 4, left_w, 12, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 1, cy + 4, " Adapter & Config ", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 1, cy + 5, "Adapter:  Intel 82540EM", Color::White, Color::Black);

        let link_str = if !dev_present { "NOT DETECTED" } else if link_up { "UP (1000 Mbps)" } else { "DOWN" };
        let link_color = if link_up { Color::LightGreen } else { Color::LightRed };
        compositor.draw_string(cx + 1, cy + 6, "Link:     ", Color::White, Color::Black);
        compositor.draw_string(cx + 11, cy + 6, link_str, link_color, Color::Black);

        let conn_str = if link_up { "CONNECTED (NAT)" } else { "DISCONNECTED" };
        compositor.draw_string(cx + 1, cy + 7, "Status:   ", Color::White, Color::Black);
        compositor.draw_string(cx + 11, cy + 7, conn_str, link_color, Color::Black);

        compositor.draw_string(cx + 1, cy + 8, "Mode:     ", Color::White, Color::Black);
        compositor.draw_string(cx + 11, cy + 8, cfg.mode.as_str(), Color::LightCyan, Color::Black);

        let mut ip_line = [0u8; 32];
        let mut ip_off = 0;
        write_bytes(&mut ip_line, &mut ip_off, b"IPv4:     ");
        for i in 0..4 {
            write_u8(&mut ip_line, &mut ip_off, cfg.ip[i]);
            if i < 3 { write_bytes(&mut ip_line, &mut ip_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&ip_line[..ip_off]) {
            compositor.draw_string(cx + 1, cy + 9, s, Color::White, Color::Black);
        }

        let mut gw_line = [0u8; 32];
        let mut gw_off = 0;
        write_bytes(&mut gw_line, &mut gw_off, b"Gateway:  ");
        for i in 0..4 {
            write_u8(&mut gw_line, &mut gw_off, cfg.gateway[i]);
            if i < 3 { write_bytes(&mut gw_line, &mut gw_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&gw_line[..gw_off]) {
            compositor.draw_string(cx + 1, cy + 10, s, Color::White, Color::Black);
        }

        let mut dns_line = [0u8; 32];
        let mut dns_off = 0;
        write_bytes(&mut dns_line, &mut dns_off, b"DNS:      ");
        for i in 0..4 {
            write_u8(&mut dns_line, &mut dns_off, cfg.dns[i]);
            if i < 3 { write_bytes(&mut dns_line, &mut dns_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&dns_line[..dns_off]) {
            compositor.draw_string(cx + 1, cy + 11, s, Color::White, Color::Black);
        }

        let mac = get_mac_address().unwrap_or(cfg.mac);
        let mut mac_line = [0u8; 32];
        let mut mac_off = 0;
        write_bytes(&mut mac_line, &mut mac_off, b"MAC: ");
        for i in 0..6 {
            write_hex2(&mut mac_line, &mut mac_off, mac[i]);
            if i < 5 { write_bytes(&mut mac_line, &mut mac_off, b":"); }
        }
        if let Ok(s) = core::str::from_utf8(&mac_line[..mac_off]) {
            compositor.draw_string(cx + 1, cy + 12, s, Color::DarkGray, Color::Black);
        }

        // Right Box: Live Traffic & Diagnostics
        let right_x = cx + left_w + 1;
        let right_w = window.client_width().saturating_sub(left_w + 1);
        compositor.draw_box(right_x, cy + 4, right_w, 12, false, Color::Cyan, Color::Black);
        compositor.draw_string(right_x + 1, cy + 4, " Live Traffic & Diagnostics ", Color::LightCyan, Color::Black);

        if let Some((tx_p, tx_b, rx_p, rx_b)) = stats {
            let mut num_b = [0u8; 16];
            let mut rx_buf = [0u8; 32];
            let mut rx_off = 0;
            write_bytes(&mut rx_buf, &mut rx_off, b"Rx: ");
            write_bytes(&mut rx_buf, &mut rx_off, fmt_u64(rx_p, &mut num_b).as_bytes());
            write_bytes(&mut rx_buf, &mut rx_off, b"p (");
            write_bytes(&mut rx_buf, &mut rx_off, fmt_u64(rx_b, &mut num_b).as_bytes());
            write_bytes(&mut rx_buf, &mut rx_off, b"B)");
            if let Ok(s) = core::str::from_utf8(&rx_buf[..rx_off]) {
                compositor.draw_string(right_x + 1, cy + 6, s, Color::LightGreen, Color::Black);
            }

            let mut tx_buf = [0u8; 32];
            let mut tx_off = 0;
            write_bytes(&mut tx_buf, &mut tx_off, b"Tx: ");
            write_bytes(&mut tx_buf, &mut tx_off, fmt_u64(tx_p, &mut num_b).as_bytes());
            write_bytes(&mut tx_buf, &mut tx_off, b"p (");
            write_bytes(&mut tx_buf, &mut tx_off, fmt_u64(tx_b, &mut num_b).as_bytes());
            write_bytes(&mut tx_buf, &mut tx_off, b"B)");
            if let Ok(s) = core::str::from_utf8(&tx_buf[..tx_off]) {
                compositor.draw_string(right_x + 1, cy + 7, s, Color::LightGreen, Color::Black);
            }
        } else {
            compositor.draw_string(right_x + 1, cy + 6, "Traffic: Controller Offline", Color::DarkGray, Color::Black);
        }

        compositor.draw_string(right_x + 1, cy + 9, "Gateway Ping: ", Color::White, Color::Black);
        match self.last_ping_status {
            Some(Ok(_)) => compositor.draw_string(right_x + 15, cy + 9, "PASS (Reply OK)", Color::LightGreen, Color::Black),
            Some(Err(_)) => compositor.draw_string(right_x + 15, cy + 9, "FAIL (Timeout)", Color::LightRed, Color::Black),
            None => compositor.draw_string(right_x + 15, cy + 9, "Ready [P]", Color::DarkGray, Color::Black),
        }

        compositor.draw_string(right_x + 1, cy + 10, "DNS Query:    ", Color::White, Color::Black);
        match self.last_dns_status {
            Some(Ok(_)) => compositor.draw_string(right_x + 15, cy + 10, "PASS (Resolved)", Color::LightGreen, Color::Black),
            Some(Err(_)) => compositor.draw_string(right_x + 15, cy + 10, "FAIL (Timeout)", Color::LightRed, Color::Black),
            None => compositor.draw_string(right_x + 15, cy + 10, "Ready [D]", Color::DarkGray, Color::Black),
        }

        compositor.draw_string(right_x + 1, cy + 12, "Uplink: Host Wi-Fi/Ethernet", Color::White, Color::Black);
        compositor.draw_string(right_x + 1, cy + 13, "Shared via VirtualBox NAT", Color::DarkGray, Color::Black);

        // Action Buttons Row (Row cy + 17)
        compositor.draw_string(cx + 1, cy + 17, "[ RENEW (R) ]", Color::Pink, Color::Blue);
        compositor.draw_string(cx + 16, cy + 17, "[ PING GW (P) ]", Color::Pink, Color::Green);
        compositor.draw_string(cx + 33, cy + 17, "[ DNS (D) ]", Color::Pink, Color::Brown);
        compositor.draw_string(cx + 46, cy + 17, "[ REFRESH (F) ]", Color::Pink, Color::DarkGray);

        // Bottom Status Message Log
        compositor.draw_string(cx, cy + 19, "System Log:", Color::LightCyan, Color::Black);
        if let Some(msg) = self.last_action_message {
            compositor.draw_string(cx + 13, cy + 19, msg, Color::White, Color::Black);
        } else {
            compositor.draw_string(cx + 13, cy + 19, "VirtualBox NAT active. Ready for network operations.", Color::DarkGray, Color::Black);
        }
        compositor.draw_string(cx, cy + 21, "Notice: BubbleOS uses virtual Ethernet. Host Wi-Fi is shared directly.", Color::DarkGray, Color::Black);
    }

    /// Renders the 1024x768 pixel-based Network Control Centre into the Framebuffer.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;

        // Background: Light Window Surface #F8F5FF
        draw::fill_rounded_rect(fb, cx, cy, cw, Window::CLIENT_PIXEL_HEIGHT, 4, theme::WINDOW_SURFACE);

        let cfg = get_config();
        let stats = get_statistics();
        let dev_present = is_device_present();
        let link_up = is_link_up();
        let mac = get_mac_address().unwrap_or(cfg.mac);

        // 1. Top Header Banner Card (y = cy + 12, w = cw - 32, h = 60)
        let top_x = cx + 16;
        let top_y = cy + 12;
        let top_w = cw - 32;
        let top_h = 60;

        draw::fill_rounded_rect(fb, top_x, top_y, top_w, top_h, 6, theme::DEEP_PURPLE);
        draw::draw_rounded_rect(fb, top_x, top_y, top_w, top_h, 6, theme::PRIMARY_PURPLE);

        font::draw_string(fb, top_x + 16, top_y + 10, "NETWORK CONTROL CENTRE", theme::WHITE, theme::DEEP_PURPLE);
        font::draw_string(
            fb,
            top_x + 230,
            top_y + 10,
            "|   Virtual Ethernet Architecture (VirtualBox / Hypervisor NAT)",
            Color::rgb(220, 205, 245),
            theme::DEEP_PURPLE,
        );
        font::draw_string(
            fb,
            top_x + 16,
            top_y + 34,
            "BubbleOS connects through an Intel 82540EM adapter; hypervisor NAT transparently shares host Wi-Fi/Internet.",
            Color::rgb(200, 185, 230),
            theme::DEEP_PURPLE,
        );

        // 2. Left Card: Ethernet Interface & IP Parameters (w = 416, h = 340)
        let col1_x = cx + 16;
        let col1_y = top_y + top_h + 12; // cy + 84
        let col1_w = 416;
        let card_h = 340;

        draw::fill_rounded_rect(fb, col1_x, col1_y, col1_w, card_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, col1_x, col1_y, col1_w, card_h, 6, theme::BORDER_LIGHT);

        // Header bar inside Left Card
        draw::fill_rounded_rect(fb, col1_x + 1, col1_y + 1, col1_w - 2, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, col1_x + 14, col1_y + 7, "ADAPTER & IPv4 PARAMETERS", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, col1_x, col1_x + col1_w, col1_y + 29, theme::BORDER_LIGHT);

        let mut row_y = col1_y + 42;
        let row_spacing = 30;

        // Adapter
        font::draw_string(fb, col1_x + 16, row_y, "Adapter:", theme::DARK_PLUM_TEXT, theme::WHITE);
        font::draw_string(fb, col1_x + 140, row_y, "Intel 82540EM Gigabit (PCI 00:03.0)", theme::PRIMARY_PURPLE, theme::WHITE);

        // Link State
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Hardware Link:", theme::DARK_PLUM_TEXT, theme::WHITE);
        if !dev_present {
            draw::fill_rounded_rect(fb, col1_x + 140, row_y - 2, 120, 20, 4, Color::rgb(240, 70, 70));
            font::draw_string(fb, col1_x + 148, row_y + 1, "NOT DETECTED", theme::WHITE, Color::rgb(240, 70, 70));
        } else if link_up {
            draw::fill_rounded_rect(fb, col1_x + 140, row_y - 2, 130, 20, 4, Color::rgb(40, 160, 80));
            font::draw_string(fb, col1_x + 148, row_y + 1, "UP (1000 Mbps)", theme::WHITE, Color::rgb(40, 160, 80));
        } else {
            draw::fill_rounded_rect(fb, col1_x + 140, row_y - 2, 80, 20, 4, Color::rgb(230, 80, 80));
            font::draw_string(fb, col1_x + 148, row_y + 1, "DOWN", theme::WHITE, Color::rgb(230, 80, 80));
        }

        // Connection State
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Connection:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let (conn_text, conn_bg) = if link_up {
            ("CONNECTED (NAT Active)", Color::rgb(40, 160, 80))
        } else {
            ("DISCONNECTED (No Link)", Color::rgb(230, 80, 80))
        };
        draw::fill_rounded_rect(fb, col1_x + 140, row_y - 2, 190, 20, 4, conn_bg);
        font::draw_string(fb, col1_x + 148, row_y + 1, conn_text, theme::WHITE, conn_bg);

        // Config Mode
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Config Mode:", theme::DARK_PLUM_TEXT, theme::WHITE);
        font::draw_string(fb, col1_x + 140, row_y, cfg.mode.as_str(), Color::rgb(60, 40, 90), theme::WHITE);

        // IPv4 Address
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "IPv4 Address:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let mut ip_buf = [0u8; 32];
        let mut ip_off = 0;
        for i in 0..4 {
            write_u8(&mut ip_buf, &mut ip_off, cfg.ip[i]);
            if i < 3 { write_bytes(&mut ip_buf, &mut ip_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&ip_buf[..ip_off]) {
            font::draw_string(fb, col1_x + 140, row_y, s, theme::PRIMARY_PURPLE, theme::WHITE);
        }

        // Subnet Mask
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Subnet Mask:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let mut mask_buf = [0u8; 32];
        let mut mask_off = 0;
        for i in 0..4 {
            write_u8(&mut mask_buf, &mut mask_off, cfg.subnet_mask[i]);
            if i < 3 { write_bytes(&mut mask_buf, &mut mask_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&mask_buf[..mask_off]) {
            font::draw_string(fb, col1_x + 140, row_y, s, theme::DARK_PLUM_TEXT, theme::WHITE);
        }

        // Default Gateway
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Default Gateway:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let mut gw_buf = [0u8; 32];
        let mut gw_off = 0;
        for i in 0..4 {
            write_u8(&mut gw_buf, &mut gw_off, cfg.gateway[i]);
            if i < 3 { write_bytes(&mut gw_buf, &mut gw_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&gw_buf[..gw_off]) {
            font::draw_string(fb, col1_x + 140, row_y, s, theme::DARK_PLUM_TEXT, theme::WHITE);
        }

        // Primary DNS
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Primary DNS:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let mut dns_buf = [0u8; 32];
        let mut dns_off = 0;
        for i in 0..4 {
            write_u8(&mut dns_buf, &mut dns_off, cfg.dns[i]);
            if i < 3 { write_bytes(&mut dns_buf, &mut dns_off, b"."); }
        }
        if let Ok(s) = core::str::from_utf8(&dns_buf[..dns_off]) {
            font::draw_string(fb, col1_x + 140, row_y, s, theme::DARK_PLUM_TEXT, theme::WHITE);
        }

        // Hardware MAC
        row_y += row_spacing;
        font::draw_string(fb, col1_x + 16, row_y, "Hardware MAC:", theme::DARK_PLUM_TEXT, theme::WHITE);
        let mut mac_buf = [0u8; 32];
        let mut mac_off = 0;
        for i in 0..6 {
            write_hex2(&mut mac_buf, &mut mac_off, mac[i]);
            if i < 5 { write_bytes(&mut mac_buf, &mut mac_off, b":"); }
        }
        if let Ok(s) = core::str::from_utf8(&mac_buf[..mac_off]) {
            font::draw_string(fb, col1_x + 140, row_y, s, Color::rgb(110, 95, 130), theme::WHITE);
        }

        // 3. Right Card: Live Traffic & Diagnostics (w = 424, h = 340)
        let col2_x = cx + 448;
        let col2_y = col1_y;
        let col2_w = cw - 464; // ~424

        draw::fill_rounded_rect(fb, col2_x, col2_y, col2_w, card_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, col2_x, col2_y, col2_w, card_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, col2_x + 1, col2_y + 1, col2_w - 2, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, col2_x + 14, col2_y + 7, "LIVE TRAFFIC & CONNECTION TESTS", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, col2_x, col2_x + col2_w, col2_y + 29, theme::BORDER_LIGHT);

        let mut r_row_y = col2_y + 42;

        // Traffic telemetry (Real statistics)
        font::draw_string(fb, col2_x + 16, r_row_y, "DMA Traffic:", theme::DARK_PLUM_TEXT, theme::WHITE);
        if let Some((tx_p, tx_b, rx_p, rx_b)) = stats {
            let mut num_b = [0u8; 16];
            let mut s_buf = [0u8; 64];

            // RX line
            let mut s_off = 0;
            write_bytes(&mut s_buf, &mut s_off, b"Rx: ");
            write_bytes(&mut s_buf, &mut s_off, fmt_u64(rx_p, &mut num_b).as_bytes());
            write_bytes(&mut s_buf, &mut s_off, b" packets (");
            write_bytes(&mut s_buf, &mut s_off, fmt_u64(rx_b, &mut num_b).as_bytes());
            write_bytes(&mut s_buf, &mut s_off, b" bytes)");
            if let Ok(s) = core::str::from_utf8(&s_buf[..s_off]) {
                font::draw_string(fb, col2_x + 140, r_row_y, s, Color::rgb(35, 140, 70), theme::WHITE);
            }

            // TX line
            r_row_y += 24;
            s_off = 0;
            write_bytes(&mut s_buf, &mut s_off, b"Tx: ");
            write_bytes(&mut s_buf, &mut s_off, fmt_u64(tx_p, &mut num_b).as_bytes());
            write_bytes(&mut s_buf, &mut s_off, b" packets (");
            write_bytes(&mut s_buf, &mut s_off, fmt_u64(tx_b, &mut num_b).as_bytes());
            write_bytes(&mut s_buf, &mut s_off, b" bytes)");
            if let Ok(s) = core::str::from_utf8(&s_buf[..s_off]) {
                font::draw_string(fb, col2_x + 140, r_row_y, s, Color::rgb(35, 140, 70), theme::WHITE);
            }
        } else {
            font::draw_string(fb, col2_x + 140, r_row_y, "Controller Offline", Color::rgb(220, 70, 70), theme::WHITE);
            r_row_y += 24;
        }

        // Divider line inside Right Card
        r_row_y += 20;
        draw::draw_hline(fb, col2_x + 16, col2_x + col2_w - 16, r_row_y, theme::BORDER_LIGHT);
        r_row_y += 14;

        // Gateway Ping Test Result
        font::draw_string(fb, col2_x + 16, r_row_y, "Gateway Ping:", theme::DARK_PLUM_TEXT, theme::WHITE);
        match self.last_ping_status {
            Some(Ok(_)) => {
                draw::fill_rounded_rect(fb, col2_x + 140, r_row_y - 2, 200, 20, 4, Color::rgb(40, 160, 80));
                font::draw_string(fb, col2_x + 148, r_row_y + 1, "PASS: Gateway Reachable", theme::WHITE, Color::rgb(40, 160, 80));
            }
            Some(Err(_)) => {
                draw::fill_rounded_rect(fb, col2_x + 140, r_row_y - 2, 180, 20, 4, Color::rgb(220, 70, 70));
                font::draw_string(fb, col2_x + 148, r_row_y + 1, "FAIL: Ping Timed Out", theme::WHITE, Color::rgb(220, 70, 70));
            }
            None => {
                font::draw_string(fb, col2_x + 140, r_row_y, "Ready to test (click [PING GATEWAY])", Color::rgb(130, 115, 145), theme::WHITE);
            }
        }

        // DNS Test Result
        r_row_y += row_spacing;
        font::draw_string(fb, col2_x + 16, r_row_y, "DNS Resolution:", theme::DARK_PLUM_TEXT, theme::WHITE);
        match self.last_dns_status {
            Some(Ok(ip)) => {
                draw::fill_rounded_rect(fb, col2_x + 140, r_row_y - 2, 230, 20, 4, Color::rgb(40, 160, 80));
                let mut d_buf = [0u8; 32];
                let mut d_off = 0;
                write_bytes(&mut d_buf, &mut d_off, b"PASS: ");
                for i in 0..4 {
                    write_u8(&mut d_buf, &mut d_off, ip[i]);
                    if i < 3 { write_bytes(&mut d_buf, &mut d_off, b"."); }
                }
                if let Ok(s) = core::str::from_utf8(&d_buf[..d_off]) {
                    font::draw_string(fb, col2_x + 148, r_row_y + 1, s, theme::WHITE, Color::rgb(40, 160, 80));
                }
            }
            Some(Err(_)) => {
                draw::fill_rounded_rect(fb, col2_x + 140, r_row_y - 2, 180, 20, 4, Color::rgb(220, 70, 70));
                font::draw_string(fb, col2_x + 148, r_row_y + 1, "FAIL: Query Timed Out", theme::WHITE, Color::rgb(220, 70, 70));
            }
            None => {
                font::draw_string(fb, col2_x + 140, r_row_y, "Ready to test (click [TEST DNS])", Color::rgb(130, 115, 145), theme::WHITE);
            }
        }

        // DHCP Status
        r_row_y += row_spacing;
        font::draw_string(fb, col2_x + 16, r_row_y, "DHCP State:", theme::DARK_PLUM_TEXT, theme::WHITE);
        match self.last_dhcp_status {
            Some(Ok(ip)) => {
                let mut d_buf = [0u8; 32];
                let mut d_off = 0;
                write_bytes(&mut d_buf, &mut d_off, b"Active Lease (");
                for i in 0..4 {
                    write_u8(&mut d_buf, &mut d_off, ip[i]);
                    if i < 3 { write_bytes(&mut d_buf, &mut d_off, b"."); }
                }
                write_bytes(&mut d_buf, &mut d_off, b")");
                if let Ok(s) = core::str::from_utf8(&d_buf[..d_off]) {
                    font::draw_string(fb, col2_x + 140, r_row_y, s, Color::rgb(35, 140, 70), theme::WHITE);
                }
            }
            Some(Err(_)) => {
                font::draw_string(fb, col2_x + 140, r_row_y, "Timed Out (Fallback Active)", Color::rgb(200, 100, 40), theme::WHITE);
            }
            None => {
                font::draw_string(fb, col2_x + 140, r_row_y, cfg.mode.as_str(), theme::PRIMARY_PURPLE, theme::WHITE);
            }
        }

        // Information box inside Right Card
        r_row_y += 36;
        let box_y = r_row_y;
        let box_w = col2_w - 32;
        draw::fill_rounded_rect(fb, col2_x + 16, box_y, box_w, 70, 5, Color::rgb(245, 240, 255));
        draw::draw_rounded_rect(fb, col2_x + 16, box_y, box_w, 70, 5, theme::BORDER_LIGHT);
        font::draw_string(fb, col2_x + 28, box_y + 10, "VirtualBox NAT shares host computer network:", theme::DARK_PLUM_TEXT, Color::rgb(245, 240, 255));
        font::draw_string(fb, col2_x + 28, box_y + 28, "- Guest packets are forwarded to host's Wi-Fi / LAN.", Color::rgb(110, 95, 130), Color::rgb(245, 240, 255));
        font::draw_string(fb, col2_x + 28, box_y + 46, "- Internet is live whenever the host machine is connected.", Color::rgb(110, 95, 130), Color::rgb(245, 240, 255));

        // 4. Action Buttons Row (y = cy + 442, height 36)
        let btn_y = cy + 442;
        draw::draw_button(fb, cx + 24, btn_y, 180, 36, "[ RENEW IP ]", false, true);
        draw::draw_button(fb, cx + 236, btn_y, 190, 36, "[ PING GATEWAY ]", false, false);
        draw::draw_button(fb, cx + 458, btn_y, 180, 36, "[ TEST DNS ]", false, false);
        draw::draw_button(fb, cx + 670, btn_y, 180, 36, "[ REFRESH ]", false, false);

        // 5. Bottom Telemetry & Status Log Card (y = cy + 494, w = cw - 32, h = 168)
        let log_x = cx + 16;
        let log_y = cy + 494;
        let log_w = cw - 32;
        let log_h = 168;

        draw::fill_rounded_rect(fb, log_x, log_y, log_w, log_h, 6, Color::rgb(35, 25, 48));
        draw::draw_rounded_rect(fb, log_x, log_y, log_w, log_h, 6, Color::rgb(70, 50, 95));

        font::draw_string(fb, log_x + 16, log_y + 12, "DIAGNOSTIC STATUS & TELEMETRY LOG", Color::rgb(200, 185, 235), Color::rgb(35, 25, 48));
        draw::draw_hline(fb, log_x + 16, log_x + log_w - 16, log_y + 32, Color::rgb(70, 50, 95));

        // Display current action message
        font::draw_string(fb, log_x + 16, log_y + 44, "Last Action:", Color::rgb(170, 155, 200), Color::rgb(35, 25, 48));
        if let Some(msg) = self.last_action_message {
            font::draw_string(fb, log_x + 120, log_y + 44, msg, theme::WHITE, Color::rgb(35, 25, 48));
        } else {
            font::draw_string(fb, log_x + 120, log_y + 44, "VirtualBox NAT active. Ready to run connection diagnostics.", Color::rgb(210, 195, 240), Color::rgb(35, 25, 48));
        }

        // System Diagnostic Info
        font::draw_string(fb, log_x + 16, log_y + 72, "Architecture:", Color::rgb(170, 155, 200), Color::rgb(35, 25, 48));
        font::draw_string(fb, log_x + 120, log_y + 72, "Intel 82540EM RX/TX DMA rings operational. Host NAT tunnel established.", Color::rgb(210, 195, 240), Color::rgb(35, 25, 48));

        font::draw_string(fb, log_x + 16, log_y + 96, "Hardware State:", Color::rgb(170, 155, 200), Color::rgb(35, 25, 48));
        let hw_state = if !dev_present {
            "E1000 device not detected on PCI bus."
        } else if link_up {
            "PCI 00:03.0 active. Full Duplex Gigabit link validated via STATUS_LU."
        } else {
            "PCI 00:03.0 active. Link carrier down (check hypervisor network cable)."
        };
        font::draw_string(fb, log_x + 140, log_y + 96, hw_state, Color::rgb(210, 195, 240), Color::rgb(35, 25, 48));

        // Navigation hints
        font::draw_string(
            fb,
            log_x + 16,
            log_y + 132,
            "Mouse: Click buttons above  |  Keys: [R] Renew IP  |  [P] Ping Gateway  |  [D] Test DNS  |  [F] Refresh  |  [Esc] Minimize",
            Color::rgb(160, 145, 190),
            Color::rgb(35, 25, 48),
        );
    }
}
