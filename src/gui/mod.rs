//! Desktop Window Manager and Application Environment for Rust Micro-OS.
//!
//! Provides the Light Lavender desktop workspace, left-hand vertical dock,
//! active window framing, double-buffered screen compositing, and application dispatch.

pub mod apps;
pub mod compositor;
pub mod dock;
pub mod window;

use spin::Mutex;
use crate::console::vga::{self, Color};
use crate::gui::apps::{
    AboutApp, BrowserApp, FilesApp, MusicApp, NetworkApp, SettingsApp, SysMonApp, TerminalApp,
};
use crate::gui::compositor::{glyphs, Compositor};
use crate::gui::dock::AppId;
use crate::gui::window::Window;

/// Global off-screen compositor buffer allocated in BSS (bypasses 256 KiB heap).
static COMPOSITOR: Mutex<Compositor> = Mutex::new(Compositor::new());

/// Displays a minimal Light Lavender boot splash screen while the kernel initializes.
pub fn show_boot_splash(status: &str) {
    vga::init_lavender_palette();
    let mut comp = COMPOSITOR.lock();
    comp.clear(Color::White, Color::Black); // fill with Light Lavender (#E9DFFF)

    let box_x = 18;
    let box_y = 8;
    let box_w = 44;
    let box_h = 8;

    // Outer double-line purple box (#5C3C8A)
    comp.draw_box(box_x, box_y, box_w, box_h, true, Color::Cyan, Color::Black);

    // Title: RUST MICRO-OS
    comp.draw_string(box_x + 15, box_y + 1, "RUST MICRO-OS", Color::LightCyan, Color::Black);
    comp.draw_string(box_x + 14, box_y + 2, "Desktop Edition", Color::DarkGray, Color::Black);

    // Status message
    let status_x = if status.len() < box_w - 4 {
        box_x + (box_w - status.len()) / 2
    } else {
        box_x + 2
    };
    comp.draw_string(status_x, box_y + 4, status, Color::White, Color::Black);

    // Progress bar: [ ============= ]
    comp.draw_string(box_x + 10, box_y + 6, "[ ============= ]", Color::LightCyan, Color::Black);

    comp.present();
}

/// Desktop Environment State
pub struct Desktop {
    pub active_app: AppId,
    pub is_window_open: bool,
    pub terminal: TerminalApp,
    pub network: NetworkApp,
    pub browser: BrowserApp,
    pub files: FilesApp,
    pub music: MusicApp,
    pub sysmon: SysMonApp,
    pub settings: SettingsApp,
    pub about: AboutApp,
}

impl Desktop {
    pub const fn new() -> Self {
        Self {
            active_app: AppId::Terminal,
            is_window_open: false, // Boots directly into Desktop Home Screen
            terminal: TerminalApp::new(),
            network: NetworkApp::new(),
            browser: BrowserApp::new(),
            files: FilesApp::new(),
            music: MusicApp::new(),
            sysmon: SysMonApp::new(),
            settings: SettingsApp::new(),
            about: AboutApp::new(),
        }
    }

    /// Renders the current desktop state into the compositor and flushes to VGA.
    pub fn render(&mut self) {
        let mut comp = COMPOSITOR.lock();

        // 1. Draw Left Application Dock (columns 0..11)
        dock::draw_dock(&mut comp, self.active_app);

        // 2. Either Draw Active Application Window or Desktop Home Screen
        if self.is_window_open {
            let window = Window::standard(self.active_app.window_title());
            window.draw_frame(&mut comp, true);

            // Render Active Application Content inside window
            match self.active_app {
                AppId::Terminal => self.terminal.draw(&window, &mut comp),
                AppId::Network => self.network.draw(&window, &mut comp),
                AppId::Browser => self.browser.draw(&window, &mut comp),
                AppId::Files => self.files.draw(&window, &mut comp),
                AppId::Music => self.music.draw(&window, &mut comp),
                AppId::SysMon => self.sysmon.draw(&window, &mut comp),
                AppId::Settings => self.settings.draw(&window, &mut comp),
                AppId::About => self.about.draw(&window, &mut comp),
            }
        } else {
            // Draw Clean Light Lavender Wallpaper & Home Screen Dashboard
            self.draw_home_screen(&mut comp);
        }

        // 3. Atomic Present to VGA Text Buffer 0xb8000
        comp.present();
    }

    /// Draws the default Desktop Home Screen with Light Lavender wallpaper and dashboard card.
    fn draw_home_screen(&self, comp: &mut Compositor) {
        // Clear workspace area (columns 12..79, rows 0..24) with Light Lavender (#E9DFFF)
        comp.fill_rect(12, 0, 68, 25, b' ', Color::White, Color::Black);

        // Top Status Header (Row 1)
        comp.draw_string(14, 1, "RUST MICRO-OS", Color::LightCyan, Color::Black);
        comp.draw_string(28, 1, "|", Color::DarkGray, Color::Black);
        comp.draw_string(30, 1, "Desktop Environment", Color::White, Color::Black);
        comp.draw_string(50, 1, "|", Color::DarkGray, Color::Black);
        comp.draw_string(52, 1, "User: maya", Color::LightGreen, Color::Black);

        // Horizontal Divider Line (Row 2)
        for col in 13..79 {
            comp.put_char(col, 2, glyphs::HORIZONTAL, Color::DarkGray, Color::Black);
        }

        // Central Welcome & Application Dashboard Card (columns 16..75, width 60, rows 4..20)
        let cx = 16;
        let cy = 4;
        let cw = 60;
        let ch = 17;

        comp.draw_box(cx, cy, cw, ch, true, Color::Cyan, Color::Black);

        // Card Header Title
        comp.draw_string(cx + 18, cy + 1, "DESKTOP HOME SCREEN", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 14, cy + 2, "Light Lavender System Workspace", Color::DarkGray, Color::Black);
        comp.draw_string(cx + 2, cy + 3, "--------------------------------------------------------", Color::DarkGray, Color::Black);

        // Information lines
        comp.draw_string(cx + 4, cy + 5, "Welcome to Rust Micro-OS, maya!", Color::White, Color::Black);
        comp.draw_string(cx + 4, cy + 6, "Press any number [1-8] or click an icon to launch an app:", Color::DarkGray, Color::Black);

        // 8 Application Shortcuts
        comp.draw_string(cx + 4, cy + 8, "[1] Terminal", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 8, "- Shell prompt & 'wifi' diagnostics", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 9, "[2] Network", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 9, "- Connection & Wi-Fi Control Center", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 10, "[3] Browser", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 10, "- Single-Tab HTTP/1.1 Web Client", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 11, "[4] Files", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 11, "- UFS Directory & File Explorer", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 12, "[5] Music", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 12, "- 48 kHz Stereo AC97 DMA Player", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 13, "[6] SysMon", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 13, "- CPU, Memory & Task Monitor", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 14, "[7] Settings", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 14, "- Display Theme & Audio Setup", Color::White, Color::Black);

        comp.draw_string(cx + 4, cy + 15, "[8] About", Color::LightCyan, Color::Black);
        comp.draw_string(cx + 17, cy + 15, "- OS Identity & Architecture", Color::White, Color::Black);

        // Bottom Navigation Bar (Row 23)
        comp.draw_string(14, 23, "[1-8] Launch App  |  [Tab] Cycle Apps  |  [Esc] Minimize Window", Color::LightCyan, Color::Black);
    }
}

/// Global desktop state instance.
static DESKTOP: Mutex<Desktop> = Mutex::new(Desktop::new());

/// Launches the interactive desktop environment.
///
/// Runs an event loop processing keyboard inputs:
/// - `Tab`: Cycle to next application
/// - `1`..`8`: Directly select application
/// - `Esc`: Exit desktop and return to classic command shell
/// - Other keys: Forwarded to active application (Terminal)
pub fn run_desktop() {
    vga::init_lavender_palette();

    // Ensure terminal initialized
    {
        let mut dt = DESKTOP.lock();
        if dt.terminal.line_count == 0 {
            dt.terminal.init();
        }
        dt.terminal.should_exit_desktop = false;
        crate::info!("[DESKTOP] Launched window manager. Active application: {}", dt.active_app.title());
        dt.render();
    }

    // Enable CPU interrupts for keyboard and timer delivery
    x86_64::instructions::interrupts::enable();

    let mut last_render_tick = crate::timer::get_ticks();
    let mut last_audio_tick = crate::timer::get_ticks();

    loop {
        // 1. Advance audio playback tick with hardware timer ticks
        let now_tick = crate::timer::get_ticks();
        if now_tick != last_audio_tick {
            let elapsed = (now_tick.saturating_sub(last_audio_tick)) as u32;
            last_audio_tick = now_tick;
            for _ in 0..elapsed.min(10) {
                crate::audio::tick();
            }
        } else {
            // Continuous polling tick to service AC97 DMA ring
            crate::audio::tick();
        }

        // 2. Poll for keyboard input (non-blocking)
        if let Some(key) = crate::console::try_read_byte() {
            let mut dt = DESKTOP.lock();

            match key {
                b'\x1b' => {
                    // Esc: Minimize active window and return to Desktop Home Screen
                    if dt.is_window_open {
                        dt.is_window_open = false;
                        crate::info!("[DESKTOP] Minimized window to Desktop Home Screen.");
                        crate::info!("[DESKTOP] Exited window manager via Escape key.");
                        crate::info!("[DESKTOP] Returned to interactive CLI shell.");
                        crate::serial_print!("micro-os> ");
                    } else {
                        // Already on home screen: exit to CLI
                        crate::info!("[DESKTOP] Exited window manager via Escape key.");
                        crate::info!("[DESKTOP] Returned to interactive CLI shell.");
                        crate::serial_print!("micro-os> ");
                        crate::audio::AUDIO_ENGINE.lock().pause();
                        break;
                    }
                }
                b'\t' => {
                    // Tab: Switch to next application and open window
                    dt.active_app = dt.active_app.next();
                    dt.is_window_open = true;
                    crate::info!("[DESKTOP] Switched active application to: {}", dt.active_app.title());
                }
                b'1'..=b'8' => {
                    // Quick-switch via number keys 1..8
                    let target_app = AppId::from_index(key - b'1');
                    if dt.is_window_open && dt.active_app == AppId::Terminal && dt.terminal.input_len > 0 {
                        // If user is actively typing a command in Terminal, forward the digit
                        dt.terminal.handle_key(key);
                    } else {
                        dt.active_app = target_app;
                        dt.is_window_open = true;
                        crate::info!("[DESKTOP] Switched active application to: {}", dt.active_app.title());
                    }
                }
                b'\n' => {
                    if !dt.is_window_open {
                        // Pressing Enter on Desktop Home Screen opens selected application
                        dt.is_window_open = true;
                        crate::info!("[DESKTOP] Opened active application: {}", dt.active_app.title());
                    } else {
                        match dt.active_app {
                            AppId::Terminal => {
                                dt.terminal.handle_key(key);
                                if dt.terminal.should_exit_desktop {
                                    dt.terminal.should_exit_desktop = false;
                                    dt.is_window_open = false;
                                    crate::info!("[DESKTOP] Closed Terminal window, returned to Home Screen.");
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    if dt.is_window_open {
                        // Forward input to active window
                        match dt.active_app {
                            AppId::Terminal => {
                                dt.terminal.handle_key(key);
                                if dt.terminal.should_exit_desktop {
                                    dt.terminal.should_exit_desktop = false;
                                    dt.is_window_open = false;
                                    crate::info!("[DESKTOP] Closed Terminal window, returned to Home Screen.");
                                }
                            }
                            AppId::Music => {
                                dt.music.handle_key(key);
                            }
                            _ => {}
                        }
                    } else {
                        // On Desktop Home Screen: typing a character opens Terminal and inputs it
                        if key.is_ascii_graphic() || key == b' ' {
                            dt.active_app = AppId::Terminal;
                            dt.is_window_open = true;
                            dt.terminal.handle_key(key);
                        }
                    }
                }
            }

            // Re-render desktop
            dt.render();
            last_render_tick = crate::timer::get_ticks();
        } else {
            // Check if audio is actively playing: re-render progress bar every 10 ticks (100 ms)
            let current_tick = crate::timer::get_ticks();
            let is_audio_playing = crate::audio::AUDIO_ENGINE.lock().state == crate::audio::PlaybackState::Playing;

            if is_audio_playing && current_tick >= last_render_tick + 10 {
                let mut dt = DESKTOP.lock();
                if dt.active_app == AppId::Music {
                    dt.render();
                }
                last_render_tick = current_tick;
            }

            // High-frequency responsive spin loop ensuring immediate key delivery
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
    }

    // Restore standard CLI VGA screen
    vga::clear_screen();
    vga::set_color(Color::White, Color::Black);
    crate::info!("[DESKTOP] Returned to interactive CLI shell.");
}
