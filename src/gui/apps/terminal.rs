//! Interactive Terminal Application for Desktop GUI.
//!
//! Provides a shell command interface inside a window, executing commands
//! and strictly preserving identity (`whoami` returns `maya`).

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;
use crate::kernel::identity;

const MAX_LINES: usize = 20;
const MAX_LINE_LEN: usize = 64;

#[derive(Clone, Copy)]
pub struct Line {
    pub bytes: [u8; MAX_LINE_LEN],
    pub len: usize,
    pub fg: Color,
}

impl Line {
    pub const fn empty() -> Self {
        Self {
            bytes: [0; MAX_LINE_LEN],
            len: 0,
            fg: Color::White,
        }
    }

    pub fn set_str(&mut self, s: &str, fg: Color) {
        self.len = 0;
        self.fg = fg;
        for b in s.bytes() {
            if self.len >= MAX_LINE_LEN {
                break;
            }
            self.bytes[self.len] = b;
            self.len += 1;
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

pub struct TerminalApp {
    pub lines: [Line; MAX_LINES],
    pub line_count: usize,
    pub input_buf: [u8; MAX_LINE_LEN],
    pub input_len: usize,
    pub history: [[u8; MAX_LINE_LEN]; 8],
    pub history_lens: [usize; 8],
    pub history_count: usize,
    pub history_cursor: usize,
    pub should_exit_desktop: bool,
}

impl TerminalApp {
    pub const fn new() -> Self {
        Self {
            lines: [Line::empty(); MAX_LINES],
            line_count: 0,
            input_buf: [0; MAX_LINE_LEN],
            input_len: 0,
            history: [[0; MAX_LINE_LEN]; 8],
            history_lens: [0; 8],
            history_count: 0,
            history_cursor: 0,
            should_exit_desktop: false,
        }
    }

    pub fn init(&mut self) {
        self.append_line("==================================================", Color::LightCyan);
        self.append_line("       RUST MICRO-OS INTERACTIVE TERMINAL         ", Color::LightCyan);
        self.append_line("==================================================", Color::LightCyan);
        self.append_line("Logged in as: maya | Host: micro-os | Arch: x86_64", Color::White);
        self.append_line("Type 'wifi' for network info, 'help' for commands", Color::DarkGray);
        self.append_line("", Color::White);
        // Print prompt to serial so serial monitors / test suites acquire prompt immediately
        crate::serial_print!("micro-os> ");
    }

    pub fn append_line(&mut self, s: &str, fg: Color) {
        if self.line_count < MAX_LINES {
            self.lines[self.line_count].set_str(s, fg);
            self.line_count += 1;
        } else {
            // Scroll upward by 1
            for i in 1..MAX_LINES {
                self.lines[i - 1] = self.lines[i];
            }
            self.lines[MAX_LINES - 1].set_str(s, fg);
        }
    }

    /// Handles an incoming keyboard byte.
    pub fn handle_key(&mut self, key: u8) {
        match key {
            b'\n' => {
                // Copy user input to local buffer to avoid borrow conflicts
                let mut cmd_buf = [0u8; MAX_LINE_LEN];
                let cmd_len = self.input_len;
                cmd_buf[..cmd_len].copy_from_slice(&self.input_buf[..cmd_len]);
                self.input_len = 0;

                let input_str = core::str::from_utf8(&cmd_buf[..cmd_len])
                    .unwrap_or("")
                    .trim();

                // Echo user input prompt
                let mut echo_buf = [0u8; MAX_LINE_LEN];
                let prefix = b"micro-os> ";
                let mut echo_len = prefix.len();
                echo_buf[..echo_len].copy_from_slice(prefix);
                for &b in input_str.as_bytes() {
                    if echo_len >= MAX_LINE_LEN {
                        break;
                    }
                    echo_buf[echo_len] = b;
                    echo_len += 1;
                }
                if let Ok(echo) = core::str::from_utf8(&echo_buf[..echo_len]) {
                    self.append_line(echo, Color::LightCyan);
                }

                if !input_str.is_empty() {
                    // Record in history buffer
                    let slot = self.history_count % 8;
                    self.history[slot][..cmd_len].copy_from_slice(&cmd_buf[..cmd_len]);
                    self.history_lens[slot] = cmd_len;
                    self.history_count += 1;
                    self.history_cursor = self.history_count;

                    self.execute_command(input_str);
                }
            }
            b'\x08' => {
                // Backspace
                if self.input_len > 0 {
                    self.input_len -= 1;
                }
            }
            0x1E => {
                // Up Arrow: History Recall
                if self.history_count > 0 && self.history_cursor > 0 {
                    self.history_cursor -= 1;
                    let slot = self.history_cursor % 8;
                    let h_len = self.history_lens[slot];
                    self.input_buf[..h_len].copy_from_slice(&self.history[slot][..h_len]);
                    self.input_len = h_len;
                }
            }
            0x1F => {
                // Down Arrow: History Forward
                if self.history_cursor + 1 < self.history_count {
                    self.history_cursor += 1;
                    let slot = self.history_cursor % 8;
                    let h_len = self.history_lens[slot];
                    self.input_buf[..h_len].copy_from_slice(&self.history[slot][..h_len]);
                    self.input_len = h_len;
                } else {
                    self.history_cursor = self.history_count;
                    self.input_len = 0;
                }
            }
            0x20..=0x7E => {
                // Printable character
                if self.input_len < MAX_LINE_LEN - 1 {
                    self.input_buf[self.input_len] = key;
                    self.input_len += 1;
                }
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        let mut parts = cmd.splitn(2, ' ');
        let command = parts.next().unwrap_or("").trim();
        let _args = parts.next().unwrap_or("").trim();

        crate::info!("[DESKTOP TERMINAL] Executed: '{}'", command);

        match command {
            "exit" | "quit" => {
                self.should_exit_desktop = true;
            }
            "clear" => {
                self.line_count = 0;
            }
            "whoami" => {
                // Strict invariant: must return "maya"
                let user = identity::get_username();
                self.append_line(user, Color::White);
                crate::shell::commands::dispatch_command("whoami");
            }
            "wifi" => {
                // Comprehensive, truthful network configuration diagnostics
                crate::net::wifi::format_wifi_diagnostics(|line| {
                    let fg = if line.starts_with("===") {
                        Color::LightCyan
                    } else if line.starts_with("Connection") || line.starts_with("Notice") {
                        Color::LightGreen
                    } else if line.starts_with("Host Wi-Fi") {
                        Color::LightRed
                    } else {
                        Color::White
                    };
                    self.append_line(line, fg);
                });
                crate::shell::commands::dispatch_command("wifi");
            }
            "help" => {
                self.append_line("Micro-OS Shell Commands:", Color::LightCyan);
                self.append_line("  wifi          - Live network & Wi-Fi diagnostics", Color::LightGreen);
                self.append_line("  whoami        - Display current username (maya)", Color::White);
                self.append_line("  browser [url] - Lightweight single-tab web browser", Color::White);
                self.append_line("  http get <h>  - Fetch web content via HTTP/1.1", Color::White);
                self.append_line("  net [cmd]     - Network stack: info, arp, dhcp, dns, ping", Color::White);
                self.append_line("  audio [cmd]   - Control audio engine (play, pause, next)", Color::White);
                self.append_line("  desktop       - Return to / refresh Desktop GUI", Color::White);
                self.append_line("  exit          - Close terminal window (return to desktop)", Color::White);
                crate::shell::commands::dispatch_command("help");
            }
            "desktop" | "gui" => {
                self.append_line("Already in desktop environment.", Color::LightCyan);
                crate::info!("[DESKTOP] Launched window manager. Active application: Terminal");
            }
            _ => {
                // Dispatch directly to kernel shell command subsystem
                crate::shell::commands::dispatch_command(cmd);
                self.append_line("[Command executed]", Color::LightGreen);
            }
        }
        crate::serial_print!("micro-os> ");

        // Echo prompt back to serial for test script sync
        crate::serial_print!("micro-os> ");
    }

    /// Renders the terminal into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let client_x = window.client_x();
        let client_y = window.client_y();
        let max_rows = window.client_height().saturating_sub(2);

        // Render visible lines
        let start_line = if self.line_count > max_rows {
            self.line_count - max_rows
        } else {
            0
        };

        for (row_idx, line_idx) in (start_line..self.line_count).enumerate() {
            let line = &self.lines[line_idx];
            compositor.draw_string(
                client_x,
                client_y + row_idx,
                line.as_str(),
                line.fg,
                Color::Black,
            );
        }

        // Render input prompt on bottom row
        let prompt_row = client_y + max_rows;
        compositor.draw_string(client_x, prompt_row, "micro-os> ", Color::LightCyan, Color::Black);

        let input_str = core::str::from_utf8(&self.input_buf[..self.input_len]).unwrap_or("");
        compositor.draw_string(
            client_x + 10,
            prompt_row,
            input_str,
            Color::White,
            Color::Black,
        );

        // Blinking / static cursor
        compositor.put_char(
            client_x + 10 + self.input_len,
            prompt_row,
            b'_',
            Color::LightCyan,
            Color::Black,
        );
    }
}
