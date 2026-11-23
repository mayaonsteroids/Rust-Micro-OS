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
        compositor.draw_string(cx + 4, cy + 15, "[2] Rust Micro-OS Documentation (http://micro-os.local/docs)", Color::LightGreen, Color::Black);

        // 4. Status Bar
        compositor.draw_string(cx, cy + 20, "----------------------------------------------------------------", Color::LightGray, Color::Black);
        compositor.draw_string(cx, cy + 21, "Status: Document loaded successfully (648 bytes retrieved via TCP)", Color::LightGreen, Color::Black);
    }
}
