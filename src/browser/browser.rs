//! Single-Tab Text-Based Web Browser Engine.
//!
//! Provides interactive terminal/VGA browsing, address bar navigation,
//! page rendering with scrolling, hyperlink following, and back history.

use crate::console::read_line;
use crate::console::vga::{self, Color};
use crate::kprint;
use crate::kprintln;
use super::html::{self, ParsedPage, MAX_URL_LEN};

pub const HISTORY_CAPACITY: usize = 8;
pub const PAGE_ROWS: usize = 18;

/// History stack for back navigation within the single tab.
pub struct HistoryStack {
    urls: [[u8; MAX_URL_LEN]; HISTORY_CAPACITY],
    lens: [usize; HISTORY_CAPACITY],
    count: usize,
}

impl HistoryStack {
    pub const fn new() -> Self {
        Self {
            urls: [[0u8; MAX_URL_LEN]; HISTORY_CAPACITY],
            lens: [0usize; HISTORY_CAPACITY],
            count: 0,
        }
    }

    pub fn push(&mut self, url: &str) {
        if self.count < HISTORY_CAPACITY {
            let bytes = url.as_bytes();
            let copy_len = core::cmp::min(bytes.len(), MAX_URL_LEN);
            self.urls[self.count][..copy_len].copy_from_slice(&bytes[..copy_len]);
            self.lens[self.count] = copy_len;
            self.count += 1;
        } else {
            // Shift left to drop oldest history entry
            for i in 0..HISTORY_CAPACITY - 1 {
                self.urls[i] = self.urls[i + 1];
                self.lens[i] = self.lens[i + 1];
            }
            let bytes = url.as_bytes();
            let copy_len = core::cmp::min(bytes.len(), MAX_URL_LEN);
            self.urls[HISTORY_CAPACITY - 1][..copy_len].copy_from_slice(&bytes[..copy_len]);
            self.lens[HISTORY_CAPACITY - 1] = copy_len;
        }
    }

    pub fn pop(&mut self) -> Option<&str> {
        if self.count > 0 {
            self.count -= 1;
            let len = self.lens[self.count];
            core::str::from_utf8(&self.urls[self.count][..len]).ok()
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Single-tab web browser state.
pub struct BrowserState {
    pub current_url: [u8; MAX_URL_LEN],
    pub current_url_len: usize,
    pub history: HistoryStack,
    pub page: ParsedPage,
    pub http_status: u16,
    pub scroll_row: usize,
    pub status_msg: [u8; 80],
    pub status_msg_len: usize,
    pub is_home: bool,
}

impl BrowserState {
    pub fn new() -> Self {
        let mut s = Self {
            current_url: [0u8; MAX_URL_LEN],
            current_url_len: 0,
            history: HistoryStack::new(),
            page: ParsedPage::new(),
            http_status: 0,
            scroll_row: 0,
            status_msg: [0u8; 80],
            status_msg_len: 0,
            is_home: true,
        };
        s.load_home_page();
        s
    }

    pub fn url_str(&self) -> &str {
        if self.current_url_len == 0 {
            "about:home"
        } else {
            core::str::from_utf8(&self.current_url[..self.current_url_len]).unwrap_or("about:home")
        }
    }

    pub fn set_status(&mut self, msg: &str) {
        let bytes = msg.as_bytes();
        let copy_len = core::cmp::min(bytes.len(), 80);
        self.status_msg[..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.status_msg_len = copy_len;
    }

    pub fn status_str(&self) -> &str {
        if self.status_msg_len == 0 {
            ""
        } else {
            core::str::from_utf8(&self.status_msg[..self.status_msg_len]).unwrap_or("")
        }
    }

    /// Loads the built-in welcome and quickstart home page.
    pub fn load_home_page(&mut self) {
        self.is_home = true;
        self.current_url_len = 0;
        self.http_status = 200;
        self.scroll_row = 0;
        self.set_status("Ready. Type 'g <url>' to browse or select a link.");

        let home_html = b"<html><head><title>BubbleOS Web Browser</title></head><body>\
            <h1>BubbleOS Web Browser</h1>\
            <p>Single-tab text web browser on bare-metal x86_64.</p>\
            <hr/>\
            <p>Controls: <b>g &lt;url&gt;</b> (navigate), <b>&lt;num&gt;</b> (follow link), <b>u/d</b> (scroll), <b>b</b> (back), <b>r</b> (reload), <b>q</b> (quit)</p>\
            <hr/>\
            <p>Quickstart Destinations:</p>\
            <p><a href=\"http://10.0.2.2:8088/test\">Local Test Server (10.0.2.2:8088/test)</a></p>\
            <p><a href=\"http://example.com/\">Example Domain (example.com)</a></p>\
            </body></html>";

        self.page = html::parse_html(home_html);
    }

    /// Navigates to a new URL string.
    /// Navigates to a new URL string.
    pub fn navigate(&mut self, target_url: &str) {
        self.navigate_internal(target_url, true);
    }

    /// Internal navigation handler.
    fn navigate_internal(&mut self, target_url: &str, push_history: bool) {
        let trimmed = target_url.trim();
        if trimmed.is_empty() {
            return;
        }

        // Validate URL and reject HTTPS immediately
        let parsed = match html::parse_url(trimmed) {
            Ok(p) => p,
            Err(e) => {
                self.set_status(e);
                return;
            }
        };

        // Push current URL to history if requested and not currently on home or empty
        if push_history && self.current_url_len > 0 {
            let mut old_buf = [0u8; MAX_URL_LEN];
            let len = self.current_url_len;
            old_buf[..len].copy_from_slice(&self.current_url[..len]);
            if let Ok(s) = core::str::from_utf8(&old_buf[..len]) {
                self.history.push(s);
            }
        }

        // Format clean active URL string
        let mut clean_url = [0u8; MAX_URL_LEN];
        let mut c_len = 0;
        let append_clean = |c_len: &mut usize, s: &str, buf: &mut [u8]| {
            let b = s.as_bytes();
            let copy = core::cmp::min(b.len(), buf.len().saturating_sub(*c_len));
            buf[*c_len..*c_len + copy].copy_from_slice(&b[..copy]);
            *c_len += copy;
        };
        append_clean(&mut c_len, "http://", &mut clean_url);
        append_clean(&mut c_len, parsed.host, &mut clean_url);
        if parsed.port != 80 {
            append_clean(&mut c_len, ":", &mut clean_url);
            let mut port_str = [0u8; 6];
            let mut p = parsed.port;
            let mut p_len = 0;
            while p > 0 {
                port_str[p_len] = b'0' + (p % 10) as u8;
                p /= 10;
                p_len += 1;
            }
            for i in (0..p_len).rev() {
                append_clean(&mut c_len, core::str::from_utf8(&[port_str[i]]).unwrap_or(""), &mut clean_url);
            }
        }
        append_clean(&mut c_len, parsed.path, &mut clean_url);

        self.current_url[..c_len].copy_from_slice(&clean_url[..c_len]);
        self.current_url_len = c_len;
        self.is_home = false;
        self.scroll_row = 0;

        self.render_fetching();

        // Perform actual HTTP GET request
        let mut hp_buf = [0u8; 80];
        let target_host_port = if parsed.port != 80 {
            let mut hp_len = 0;
            let hp_b = parsed.host.as_bytes();
            let c = core::cmp::min(hp_b.len(), hp_buf.len());
            hp_buf[..c].copy_from_slice(&hp_b[..c]);
            hp_len += c;
            hp_buf[hp_len] = b':';
            hp_len += 1;
            let mut port_str = [0u8; 6];
            let mut p = parsed.port;
            let mut p_len = 0;
            while p > 0 {
                port_str[p_len] = b'0' + (p % 10) as u8;
                p /= 10;
                p_len += 1;
            }
            for i in (0..p_len).rev() {
                hp_buf[hp_len] = port_str[i];
                hp_len += 1;
            }
            core::str::from_utf8(&hp_buf[..hp_len]).unwrap_or(parsed.host)
        } else {
            parsed.host
        };

        match crate::net::http_get(target_host_port, parsed.path) {
            Ok(resp) => {
                self.http_status = resp.status_code;
                self.page = html::parse_html(&resp.body[..resp.body_len]);
                if resp.is_truncated {
                    self.set_status("Loaded (response truncated to 8192-byte safety limit)");
                } else {
                    self.set_status("Page loaded successfully.");
                }
            }
            Err(e) => {
                self.http_status = 0;
                let mut err_buf = [0u8; 512];
                let mut e_len = 0;
                let add_err = |e_len: &mut usize, s: &str, buf: &mut [u8]| {
                    let b = s.as_bytes();
                    let c = core::cmp::min(b.len(), buf.len().saturating_sub(*e_len));
                    buf[*e_len..*e_len + c].copy_from_slice(&b[..c]);
                    *e_len += c;
                };
                add_err(&mut e_len, "<html><head><title>Navigation Error</title></head><body><h1>Navigation Error</h1><p>Failed to retrieve: ", &mut err_buf);
                add_err(&mut e_len, self.url_str(), &mut err_buf);
                add_err(&mut e_len, "</p><hr/><p>Reason: ", &mut err_buf);
                add_err(&mut e_len, e, &mut err_buf);
                add_err(&mut e_len, "</p><p>Press 'b' to go back, or 'g <url>' to try another destination.</p></body></html>", &mut err_buf);

                self.page = html::parse_html(&err_buf[..e_len]);
                self.set_status(e);
            }
        }
    }

    /// Follows a link by numerical identifier.
    pub fn follow_link(&mut self, link_id: usize) {
        if link_id == 0 || link_id > self.page.link_count {
            self.set_status("Invalid link number.");
            return;
        }

        let link = self.page.links[link_id - 1];
        let link_target = link.url_str();

        let base_parsed = match html::parse_url(self.url_str()) {
            Ok(p) => p,
            Err(_) => {
                self.navigate(link_target);
                return;
            }
        };

        let mut resolved_buf = [0u8; MAX_URL_LEN];
        match html::resolve_relative_url(&base_parsed, link_target, &mut resolved_buf) {
            Ok(res_len) => {
                if let Ok(res_str) = core::str::from_utf8(&resolved_buf[..res_len]) {
                    self.navigate(res_str);
                } else {
                    self.set_status("Failed to decode resolved URL.");
                }
            }
            Err(e) => {
                self.set_status(e);
            }
        }
    }

    /// Navigates back to the preceding URL in the single-tab history.
    pub fn back(&mut self) {
        if let Some(prev_url) = self.history.pop() {
            let mut url_copy = [0u8; MAX_URL_LEN];
            let len = prev_url.len();
            url_copy[..len].copy_from_slice(prev_url.as_bytes());
            if let Ok(s) = core::str::from_utf8(&url_copy[..len]) {
                self.navigate_internal(s, false);
            }
        } else {
            self.set_status("History is empty. No previous page.");
        }
    }

    /// Reloads the active page.
    pub fn reload(&mut self) {
        if self.is_home {
            self.load_home_page();
        } else {
            let curr = self.url_str();
            let mut copy = [0u8; MAX_URL_LEN];
            let len = curr.len();
            copy[..len].copy_from_slice(curr.as_bytes());
            if let Ok(s) = core::str::from_utf8(&copy[..len]) {
                self.navigate_internal(s, false);
            }
        }
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.page.line_count.saturating_sub(PAGE_ROWS);
        if self.scroll_row < max_scroll {
            self.scroll_row = core::cmp::min(self.scroll_row + 10, max_scroll);
        } else {
            self.set_status("End of page.");
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_row > 0 {
            self.scroll_row = self.scroll_row.saturating_sub(10);
        } else {
            self.set_status("Top of page.");
        }
    }

    /// Displays loading feedback on the console.
    fn render_fetching(&self) {
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("[Browser] Fetching {}...", self.url_str());
        vga::set_color(Color::White, Color::Black);
    }

    /// Renders the complete single-tab browser UI to VGA and Serial.
    pub fn render_screen(&self) {
        vga::init_lavender_palette();
        vga::clear_screen();
        crate::serial_print!("\x1b[2J\x1b[H");

        // Row 0: Title Bar (Medium Purple on Light Lavender background)
        vga::set_color(Color::LightCyan, Color::Black);
        let title = self.page.title_str();
        kprint!("[BubbleOS Browser] - ");
        if title.len() > 40 {
            kprint!("{:.40}... ", title);
        } else {
            kprint!("{} ", title);
        }
        if self.http_status > 0 {
            kprintln!("(HTTP {}) [Tab 1/1]", self.http_status);
        } else {
            kprintln!("[Tab 1/1]");
        }

        // Row 1: Address Bar
        vga::set_color(Color::White, Color::Black);
        kprint!("URL: ");
        vga::set_color(Color::LightBlue, Color::Black);
        kprintln!("{}", self.url_str());

        // Row 2: Top Separator
        vga::set_color(Color::DarkGray, Color::Black);
        kprintln!("--------------------------------------------------------------------------------");

        // Rows 3..20 (18 content rows)
        vga::set_color(Color::White, Color::Black);
        for row in 0..PAGE_ROWS {
            let line_idx = self.scroll_row + row;
            if line_idx < self.page.line_count {
                let line_text = self.page.get_line(line_idx);
                kprintln!("{}", line_text);
            } else {
                kprintln!();
            }
        }

        // Row 21: Status Message or Truncation Notice
        let status = self.status_str();
        if !status.is_empty() {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("[STATUS] {}", status);
        } else {
            vga::set_color(Color::DarkGray, Color::Black);
            kprintln!("--------------------------------------------------------------------------------");
        }

        // Row 22: Controls Toolbar
        vga::set_color(Color::LightCyan, Color::Black);
        let total_lines = self.page.line_count;
        let current_line = if total_lines == 0 { 0 } else { self.scroll_row + 1 };
        kprintln!(
            "[G]oto [U]p [D]own [F]ollow (#) [B]ack [R]eload [Q]uit | Line {}/{} Links:{}",
            current_line, total_lines, self.page.link_count
        );

        // Row 23: Input Prompt
        vga::set_color(Color::LightGreen, Color::Black);
        kprint!("browser> ");
        vga::set_color(Color::White, Color::Black);
    }
}

/// Runs the interactive single-tab browser session.
pub fn run_browser(initial_url: Option<&str>) {
    let mut browser = BrowserState::new();

    if let Some(url) = initial_url {
        browser.navigate(url);
    }

    let mut line_buf = [0u8; 128];

    loop {
        browser.render_screen();

        let len = read_line(&mut line_buf);
        if len == 0 {
            continue;
        }

        let input = match core::str::from_utf8(&line_buf[..len]) {
            Ok(s) => s.trim(),
            Err(_) => continue,
        };

        if input.is_empty() {
            continue;
        }

        // Check for quit
        if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
            vga::set_color(Color::LightCyan, Color::Black);
            kprintln!("[Browser] Closing browser tab and returning to BubbleOS shell...");
            vga::set_color(Color::White, Color::Black);
            break;
        }

        // Navigation controls
        if input.eq_ignore_ascii_case("u") || input.eq_ignore_ascii_case("up") || input.eq_ignore_ascii_case("p") {
            browser.scroll_up();
        } else if input.eq_ignore_ascii_case("d") || input.eq_ignore_ascii_case("down") || input.eq_ignore_ascii_case("n") {
            browser.scroll_down();
        } else if input.eq_ignore_ascii_case("t") || input.eq_ignore_ascii_case("top") {
            browser.scroll_row = 0;
            browser.set_status("Top of page.");
        } else if input.eq_ignore_ascii_case("b") || input.eq_ignore_ascii_case("back") {
            browser.back();
        } else if input.eq_ignore_ascii_case("r") || input.eq_ignore_ascii_case("reload") {
            browser.reload();
        } else if input.eq_ignore_ascii_case("home") || input.eq_ignore_ascii_case("about:home") {
            browser.load_home_page();
        } else if input.starts_with("g ") || input.starts_with("G ") || input.starts_with("goto ") {
            let dest = input.splitn(2, ' ').nth(1).unwrap_or("").trim();
            browser.navigate(dest);
        } else if input.starts_with("f ") || input.starts_with("F ") || input.starts_with("follow ") {
            let arg = input.splitn(2, ' ').nth(1).unwrap_or("").trim();
            if let Some(num) = parse_dec_usize(arg) {
                browser.follow_link(num);
            } else {
                browser.set_status("Invalid link number.");
            }
        } else if let Some(num) = parse_dec_usize(input) {
            // Typing just the link number follows the link
            browser.follow_link(num);
        } else if input.contains('.') || input.starts_with("http") {
            // Direct URL input
            browser.navigate(input);
        } else {
            browser.set_status("Unknown command. Use g <url>, f <num>, u, d, b, r, or q.");
        }
    }

    // Clean screen restore on browser exit
    vga::init_lavender_palette();
    vga::clear_screen();
    crate::serial_print!("\x1b[2J\x1b[H");
}

fn parse_dec_usize(s: &str) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let mut val: usize = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val.checked_mul(10)?.checked_add((b - b'0') as usize)?;
    }
    Some(val)
}
