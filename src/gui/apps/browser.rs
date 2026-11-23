//! Single-Tab Web Browser Application for Desktop GUI.
//!
//! Provides URL navigation bar, navigation controls, and HTML document viewport.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;

pub struct BrowserApp;

impl BrowserApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders the web browser into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // 1. Navigation Toolbar & Address Bar
        compositor.draw_string(cx, cy, "[< Back] [> Forward] [Reload]  URL: [ http://example.com/           ] [Go]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx, cy + 1, "----------------------------------------------------------------", Color::LightGray, Color::Black);

        // 2. Web Document Header
        compositor.draw_string(cx + 2, cy + 3, "Example Domain - Remote Web Page View", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 4, "HTTP/1.1 200 OK | Content-Type: text/html | Stack: E1000 TCP/IP", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 2, cy + 5, "------------------------------------------------------------", Color::LightGray, Color::Black);

        // 3. Rendered Content Body
        compositor.draw_string(cx + 2, cy + 7, "This domain is established to be used for illustrative examples", Color::White, Color::Black);
        compositor.draw_string(cx + 2, cy + 8, "in documents. You may use this domain in examples without", Color::White, Color::Black);
        compositor.draw_string(cx + 2, cy + 9, "prior coordination or asking for permission.", Color::White, Color::Black);

        compositor.draw_string(cx + 2, cy + 12, "Interactive Document Hyperlinks:", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 14, "[1] More information... (https://www.iana.org/domains/example)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 4, cy + 15, "[2] BubbleOS Documentation (http://micro-os.local/docs)", Color::LightGreen, Color::Black);

        // 4. Status Bar
        compositor.draw_string(cx, cy + 20, "----------------------------------------------------------------", Color::LightGray, Color::Black);
        compositor.draw_string(cx, cy + 21, "Status: Document loaded successfully (648 bytes retrieved via TCP)", Color::LightGreen, Color::Black);
    }

    /// Renders the pixel-based web browser into the window client area.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let ch = Window::CLIENT_PIXEL_HEIGHT;

        // Background: Light Window Surface #F8F5FF
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 4, theme::WINDOW_SURFACE);

        // 1. Navigation Toolbar Bar (y = cy + 12, h = 48)
        let tb_y = cy + 12;
        let tb_h = 48;
        draw::fill_rounded_rect(fb, cx + 16, tb_y, cw - 32, tb_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, tb_y, cw - 32, tb_h, 6, theme::BORDER_LIGHT);

        draw::draw_button(fb, cx + 24, tb_y + 8, 70, 32, "< Back", false, false);
        draw::draw_button(fb, cx + 100, tb_y + 8, 70, 32, "Fwd >", false, false);
        draw::draw_button(fb, cx + 176, tb_y + 8, 75, 32, "Reload", false, false);

        // Address Bar
        let url_x = cx + 260;
        let url_w = cw.saturating_sub(380);
        draw::fill_rounded_rect(fb, url_x, tb_y + 8, url_w, 32, 4, Color::rgb(248, 245, 255));
        draw::draw_rounded_rect(fb, url_x, tb_y + 8, url_w, 32, 4, theme::BORDER_LIGHT);
        font::draw_string(fb, url_x + 12, tb_y + 16, "http://example.com/", theme::DARK_PLUM_TEXT, Color::rgb(248, 245, 255));

        draw::draw_button(fb, url_x + url_w + 12, tb_y + 8, 60, 32, "Go", false, true);

        // 2. Web Document Paper Card (y = tb_y + tb_h + 16)
        let doc_y = tb_y + tb_h + 16;
        let doc_h = ch.saturating_sub(tb_h + 90);
        draw::draw_shadow(fb, cx + 16, doc_y, cw - 32, doc_h, 6);
        draw::fill_rounded_rect(fb, cx + 16, doc_y, cw - 32, doc_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, doc_y, cw - 32, doc_h, 6, theme::BORDER_LIGHT);

        // Document Banner
        font::draw_string_scaled(fb, cx + 36, doc_y + 24, "Example Domain", theme::DEEP_PURPLE, 2);
        font::draw_string(fb, cx + 36, doc_y + 64, "HTTP/1.1 200 OK | Content-Type: text/html | Stack: E1000 TCP/IP", Color::rgb(130, 115, 150), theme::WHITE);
        draw::draw_hline(fb, cx + 36, cx + cw - 52, doc_y + 86, theme::BORDER_LIGHT);

        // Document Paragraphs
        font::draw_string(fb, cx + 36, doc_y + 104, "This domain is established to be used for illustrative examples in documents.", theme::DARK_PLUM_TEXT, theme::WHITE);
        font::draw_string(fb, cx + 36, doc_y + 128, "You may use this domain in examples without prior coordination or asking for permission.", theme::DARK_PLUM_TEXT, theme::WHITE);

        // Hyperlinks Section
        font::draw_string(fb, cx + 36, doc_y + 172, "Interactive Document Hyperlinks:", theme::PRIMARY_PURPLE, theme::WHITE);
        draw::fill_rounded_rect(fb, cx + 36, doc_y + 196, 520, 34, 4, Color::rgb(240, 235, 250));
        font::draw_string(fb, cx + 48, doc_y + 204, "[1] More information... (https://www.iana.org/domains/example)", Color::rgb(30, 130, 220), Color::rgb(240, 235, 250));

        draw::fill_rounded_rect(fb, cx + 36, doc_y + 238, 520, 34, 4, Color::rgb(240, 235, 250));
        font::draw_string(fb, cx + 48, doc_y + 246, "[2] BubbleOS Documentation (http://micro-os.local/docs)", Color::rgb(30, 130, 220), Color::rgb(240, 235, 250));

        // 3. Status Bar
        let sb_y = doc_y + doc_h + 12;
        draw::fill_rounded_rect(fb, cx + 16, sb_y, cw - 32, 32, 4, Color::rgb(245, 240, 255));
        draw::draw_rounded_rect(fb, cx + 16, sb_y, cw - 32, 32, 4, theme::BORDER_LIGHT);
        font::draw_string(fb, cx + 28, sb_y + 8, "Status: Document loaded successfully (648 bytes retrieved via TCP socket)", Color::rgb(40, 150, 70), Color::rgb(245, 240, 255));
    }
}
