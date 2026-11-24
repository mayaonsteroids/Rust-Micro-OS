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

    // Title: BUBBLEOS
    comp.draw_string(box_x + 18, box_y + 1, "BUBBLEOS", Color::LightCyan, Color::Black);
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
    pub last_music_elapsed: u32,
    pub last_music_state: crate::audio::PlaybackState,
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
            last_music_elapsed: 0,
            last_music_state: crate::audio::PlaybackState::Stopped,
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

    /// Renders the current desktop state into the compositor and flushes to VGA / Framebuffer.
    pub fn render(&mut self) {
        // 1. If graphical framebuffer is active (BGA 1024x768x32), render truecolor pixel GUI
        if crate::graphics::is_active() {
            crate::graphics::with_framebuffer(|fb| {
                self.render_pixel(fb);
                fb.present();
                let (mx, my) = crate::console::mouse::get_position();
                crate::graphics::cursor::draw_cursor(fb, mx, my);
            });
        }

        // 2. Also update compositor and VGA buffer for 100% regression compatibility
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

    /// Renders only the music progress bar, elapsed time, and status badge without a full screen redraw.
    pub fn render_music_progress(&mut self) {
        if self.is_window_open && self.active_app == AppId::Music {
            if crate::graphics::is_active() {
                crate::graphics::with_framebuffer(|fb| {
                    let window = Window::standard(self.active_app.window_title());
                    self.music.draw_pixel_progress(&window, fb);
                    fb.present();
                    let (mx, my) = crate::console::mouse::get_position();
                    crate::graphics::cursor::draw_cursor(fb, mx, my);
                });
            } else {
                let mut comp = COMPOSITOR.lock();
                let window = Window::standard(self.active_app.window_title());
                self.music.draw(&window, &mut comp);
                comp.present();
            }
        }
    }

    /// Renders the pixel-based desktop into the 1024x768 Framebuffer.
    pub fn render_pixel(&mut self, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::theme;

        // 1. Clear Desktop Workspace with Light Lavender (#E9DFFF)
        fb.clear(theme::DESKTOP_BG);

        // 2. Draw Left Application Dock (x = 0..88, y = 0..768)
        dock::draw_pixel_dock(fb, self.active_app);

        // 3. Either Draw Active Application Window or Desktop Home Screen Dashboard
        if self.is_window_open {
            let window = Window::standard(self.active_app.window_title());
            window.draw_pixel_frame(fb, true);

            // Render Active Application Content inside pixel window
            match self.active_app {
                AppId::Terminal => self.terminal.draw_pixel(&window, fb),
                AppId::Network => self.network.draw_pixel(&window, fb),
                AppId::Browser => self.browser.draw_pixel(&window, fb),
                AppId::Files => self.files.draw_pixel(&window, fb),
                AppId::Music => self.music.draw_pixel(&window, fb),
                AppId::SysMon => self.sysmon.draw_pixel(&window, fb),
                AppId::Settings => self.settings.draw_pixel(&window, fb),
                AppId::About => self.about.draw_pixel(&window, fb),
            }
        } else {
            // Draw Clean Light Lavender Wallpaper & Home Screen Dashboard
            self.draw_pixel_home_screen(fb);
        }
    }

    /// Draws the default Desktop Home Screen with Light Lavender wallpaper and dashboard card.
    fn draw_pixel_home_screen(&self, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};
        use crate::graphics::image::AppIcon;

        // Top Status Header Bar (x = 104, y = 16, w = 904, h = 40)
        let top_x = 104;
        let top_y = 16;
        let top_w = 904;
        let top_h = 40;

        draw::draw_shadow(fb, top_x, top_y, top_w, top_h, 6);
        draw::fill_rounded_rect(fb, top_x, top_y, top_w, top_h, 6, theme::WINDOW_SURFACE);
        draw::draw_rounded_rect(fb, top_x, top_y, top_w, top_h, 6, theme::BORDER_LIGHT);

        font::draw_string(fb, top_x + 16, top_y + 12, "BUBBLEOS", theme::PRIMARY_PURPLE, theme::WINDOW_SURFACE);
        font::draw_string(fb, top_x + 105, top_y + 12, "|   A little OS with big possibilities   |   User: maya   |   x86_64", theme::DARK_PLUM_TEXT, theme::WINDOW_SURFACE);
        font::draw_string(fb, top_x + top_w - 180, top_y + 12, "[Light Lavender Theme]", theme::PRIMARY_PURPLE, theme::WINDOW_SURFACE);

        // Central Welcome & Application Dashboard Card (x = 136, y = 72, w = 840, h = 668)
        let cx = 136;
        let cy = 72;
        let cw = 840;
        let ch = 668;

        draw::draw_shadow(fb, cx, cy, cw, ch, 8);
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 8, theme::WINDOW_SURFACE);
        draw::draw_rounded_rect(fb, cx, cy, cw, ch, 8, theme::BORDER_LIGHT);

        // Header Banner inside Card
        draw::fill_rounded_rect(fb, cx + 1, cy + 1, cw - 2, 60, 7, theme::DEEP_PURPLE);
        fb.fill_rect(cx + 1, cy + 30, cw - 2, 32, theme::DEEP_PURPLE);

        font::draw_string_scaled(fb, cx + 24, cy + 12, "WELCOME TO BUBBLEOS", theme::WHITE, 2);
        font::draw_string(fb, cx + 24, cy + 42, "A little OS with big possibilities - Click any icon or press [1-8] to launch", Color::rgb(225, 210, 248), theme::DEEP_PURPLE);

        // 8 Application Quick-Launch Tiles (2 columns x 4 rows)
        let col_w = 380;
        let tile_h = 100;
        let col0_x = cx + 24;
        let col1_x = cx + cw - 24 - col_w;

        let app_descriptions = [
            ("Terminal", "Interactive shell prompt, whoami, wifi diagnostics", "[1]"),
            ("Network", "Connection & Wi-Fi Control Center, E1000 Gigabit", "[2]"),
            ("Browser", "Single-Tab HTTP/1.1 Web Client, URL navigation", "[3]"),
            ("Files", "UFS Directory & File Explorer, virtual RamFS", "[4]"),
            ("Music Player", "Spotify-inspired 48 kHz AC97 DMA song player", "[5]"),
            ("System Monitor", "CPU, PIT timer, 256 KiB heap & task manager", "[6]"),
            ("Settings", "Display theme, PS/2 controller, audio setup", "[7]"),
            ("About", "OS identity, verified user maya, x86_64 specs", "[8]"),
        ];

        for (i, app) in AppId::ALL.iter().enumerate() {
            let col = i % 2;
            let row = i / 2;
            let tile_x = if col == 0 { col0_x } else { col1_x };
            let tile_y = cy + 76 + row * 114;

            let is_selected = *app == self.active_app;

            // Tile background
            let tile_bg = if is_selected { Color::rgb(245, 238, 255) } else { theme::WHITE };
            let tile_border = if is_selected { theme::PRIMARY_PURPLE } else { theme::BORDER_LIGHT };

            draw::fill_rounded_rect(fb, tile_x, tile_y, col_w, tile_h, 6, tile_bg);
            draw::draw_rounded_rect(fb, tile_x, tile_y, col_w, tile_h, 6, tile_border);

            // App Icon
            let icon = AppIcon::new(*app);
            icon.draw(fb, tile_x + 16, tile_y + 16, is_selected);

            // Title & Shortcut
            let (title, desc, shortcut) = app_descriptions[i];
            font::draw_string(fb, tile_x + 60, tile_y + 16, shortcut, theme::PRIMARY_PURPLE, tile_bg);
            font::draw_string(fb, tile_x + 88, tile_y + 16, title, theme::DEEP_PURPLE, tile_bg);

            // Description
            font::draw_string(fb, tile_x + 60, tile_y + 40, desc, theme::DARK_PLUM_TEXT, tile_bg);

            // Launch hint pill
            let pill_text = if is_selected { "Selected (Enter)" } else { "Click / Press" };
            let pill_color = if is_selected { Color::rgb(35, 140, 70) } else { Color::rgb(130, 115, 150) };
            font::draw_string(fb, tile_x + 60, tile_y + 68, pill_text, pill_color, tile_bg);
        }

        // Bottom Navigation Tips
        let nav_y = cy + ch - 34;
        font::draw_string(fb, cx + 48, nav_y, "Navigation: [1-8] Launch App  |  [Tab] Cycle Selection  |  [Enter] Open Window  |  [Esc] Minimize Window", theme::DARK_PLUM_TEXT, theme::WINDOW_SURFACE);
    }

    /// Draws the default Desktop Home Screen with Light Lavender wallpaper and dashboard card.
    fn draw_home_screen(&self, comp: &mut Compositor) {
        // Clear workspace area (columns 12..79, rows 0..24) with Light Lavender (#E9DFFF)
        comp.fill_rect(12, 0, 68, 25, b' ', Color::White, Color::Black);

        // Top Status Header (Row 1)
        comp.draw_string(14, 1, "BUBBLEOS", Color::LightCyan, Color::Black);
        comp.draw_string(23, 1, "|", Color::DarkGray, Color::Black);
        comp.draw_string(25, 1, "Desktop Environment", Color::White, Color::Black);
        comp.draw_string(45, 1, "|", Color::DarkGray, Color::Black);
        comp.draw_string(47, 1, "User: maya", Color::LightGreen, Color::Black);

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
        comp.draw_string(cx + 4, cy + 5, "Welcome to BubbleOS, maya! A little OS with big possibilities.", Color::White, Color::Black);
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

    /// Handles left mouse button click events with hit-testing across dock, window controls, and dashboard.
    pub fn handle_mouse_click(&mut self, x: usize, y: usize) {
        // 1. Left Application Dock (x: 0..88)
        if x < 88 {
            if y >= 36 && y < 36 + 8 * 86 {
                let idx = (y - 36) / 86;
                if idx < 8 {
                    let target_app = AppId::from_index(idx as u8);
                    self.active_app = target_app;
                    self.is_window_open = true;
                    crate::info!("[MOUSE] Dock click -> Activated {}", target_app.title());
                    self.render();
                    return;
                }
            }
        }

        // 2. Active Window Header Controls & Application Interactivity
        if self.is_window_open {
            // Close Button [X] at top-right (x: 965..1000, y: 20..52)
            if x >= 965 && x <= 1000 && y >= 20 && y <= 52 {
                self.is_window_open = false;
                if self.active_app == AppId::Terminal {
                    self.terminal.should_exit_desktop = false;
                }
                crate::info!("[MOUSE] Clicked Close [X] -> Returned to Home Screen.");
                self.render();
                return;
            }

            // Minimize Button [-] at top-right (x: 935..965, y: 20..52)
            if x >= 935 && x <= 965 && y >= 20 && y <= 52 {
                self.is_window_open = false;
                crate::info!("[MOUSE] Clicked Minimize [-] -> Minimized to Home Screen.");
                self.render();
                return;
            }

            // App-specific interactive mouse handling
            match self.active_app {
                AppId::Network => {
                    let cx = Window::CLIENT_PIXEL_X;
                    let cy = Window::CLIENT_PIXEL_Y;
                    let btn_y = cy + 442;
                    if y >= btn_y && y <= btn_y + 36 {
                        if x >= cx + 24 && x <= cx + 24 + 180 {
                            self.network.renew_ip();
                            crate::info!("[MOUSE] Network: Clicked Renew IP");
                            self.render();
                            return;
                        } else if x >= cx + 236 && x <= cx + 236 + 190 {
                            self.network.ping_gateway();
                            crate::info!("[MOUSE] Network: Clicked Ping Gateway");
                            self.render();
                            return;
                        } else if x >= cx + 458 && x <= cx + 458 + 180 {
                            self.network.test_dns();
                            crate::info!("[MOUSE] Network: Clicked Test DNS");
                            self.render();
                            return;
                        } else if x >= cx + 670 && x <= cx + 670 + 180 {
                            self.network.refresh();
                            crate::info!("[MOUSE] Network: Clicked Refresh");
                            self.render();
                            return;
                        }
                    }
                }
                AppId::Music => {
                    let cx = Window::CLIENT_PIXEL_X;
                    let cy = Window::CLIENT_PIXEL_Y;
                    let cw = Window::CLIENT_PIXEL_WIDTH;

                    let ctrl_y = cy + 424;
                    let bar_y = ctrl_y + 16;
                    let btn_row_y = bar_y + 36;
                    let mid_x = cx + cw / 2;

                    // Play/Pause button [mid_x - 60, mid_x + 60] (496..616), y: btn_row_y - 5 .. btn_row_y + 45
                    if x >= mid_x - 60 && x <= mid_x + 60 && y >= btn_row_y.saturating_sub(5) && y <= btn_row_y + 45 {
                        let mut audio = crate::audio::AUDIO_ENGINE.lock();
                        if audio.state == crate::audio::PlaybackState::Playing {
                            audio.pause();
                            crate::info!("[MOUSE] Music Player: Clicked Pause");
                        } else {
                            audio.play();
                            crate::info!("[MOUSE] Music Player: Clicked Play/Resume");
                        }
                        drop(audio);
                        self.render();
                        return;
                    }

                    // Prev button [mid_x - 180, mid_x - 80] (376..476), y: btn_row_y .. btn_row_y + 40
                    if x >= mid_x - 180 && x <= mid_x - 80 && y >= btn_row_y && y <= btn_row_y + 40 {
                        let mut audio = crate::audio::AUDIO_ENGINE.lock();
                        audio.prev_track();
                        crate::info!("[MOUSE] Music Player: Clicked Prev Track");
                        drop(audio);
                        self.render();
                        return;
                    }

                    // Next button [mid_x + 80, mid_x + 180] (636..736), y: btn_row_y .. btn_row_y + 40
                    if x >= mid_x + 80 && x <= mid_x + 180 && y >= btn_row_y && y <= btn_row_y + 40 {
                        let mut audio = crate::audio::AUDIO_ENGINE.lock();
                        audio.next_track();
                        crate::info!("[MOUSE] Music Player: Clicked Next Track");
                        drop(audio);
                        self.render();
                        return;
                    }

                    // Repeat button [mid_x + 200, mid_x + 320] (756..876), y: btn_row_y .. btn_row_y + 40
                    if x >= mid_x + 200 && x <= mid_x + 320 && y >= btn_row_y && y <= btn_row_y + 40 {
                        let mut audio = crate::audio::AUDIO_ENGINE.lock();
                        let rep = audio.toggle_repeat();
                        crate::info!("[MOUSE] Music Player: Clicked Repeat -> {}", if rep { "ON" } else { "OFF" });
                        drop(audio);
                        self.render();
                        return;
                    }

                    // Volume slider [vol_x, vol_x + 200] (406..606), y: btn_row_y + 54 .. btn_row_y + 74
                    let vol_x = mid_x - 150;
                    let vol_y = btn_row_y + 58;
                    if x >= vol_x && x <= vol_x + 200 && y >= vol_y.saturating_sub(4) && y <= vol_y + 16 {
                        let pct = (((x - vol_x) * 100) / 200).min(100) as u8;
                        let mut audio = crate::audio::AUDIO_ENGINE.lock();
                        audio.set_volume(pct);
                        crate::info!("[MOUSE] Music Player: Clicked Volume -> {}%", pct);
                        drop(audio);
                        self.render();
                        return;
                    }

                    // Playlist card rows: pl_start = cy + 224 + 44 = cy + 268
                    let pl_start = cy + 268;
                    if x >= cx + 24 && x <= cx + cw - 24 && y >= pl_start && y < pl_start + crate::audio::melody::PLAYLIST.len() * 38 {
                        let row_idx = (y - pl_start) / 38;
                        if row_idx < crate::audio::melody::PLAYLIST.len() {
                            let mut audio = crate::audio::AUDIO_ENGINE.lock();
                            audio.current_track_idx = row_idx;
                            audio.play();
                            crate::info!("[MOUSE] Music Player: Clicked track #{}", row_idx + 1);
                            drop(audio);
                            self.render();
                            return;
                        }
                    }
                }
                _ => {}
            }
        } else {
            // 3. Desktop Home Screen Dashboard Tiles (2 columns x 4 rows)
            let cx = 136;
            let cy = 72;
            let col_w = 380;
            let tile_h = 100;
            let col0_x = cx + 24;
            let col1_x = cx + 840 - 24 - col_w;

            for (i, app) in AppId::ALL.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let tile_x = if col == 0 { col0_x } else { col1_x };
                let tile_y = cy + 76 + row * 114;

                if x >= tile_x && x < tile_x + col_w && y >= tile_y && y < tile_y + tile_h {
                    self.active_app = *app;
                    self.is_window_open = true;
                    crate::info!("[MOUSE] Dashboard tile click -> Opened {}", app.title());
                    self.render();
                    return;
                }
            }
        }
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

    let mut last_audio_tick = crate::timer::get_ticks();
    let (mut last_mouse_x, mut last_mouse_y) = crate::console::mouse::get_position();

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

        // 2. Poll for mouse movements and button clicks (non-blocking)
        let mouse_state = crate::console::mouse::get_mouse_state();
        let mx = mouse_state.x;
        let my = mouse_state.y;

        if mx != last_mouse_x || my != last_mouse_y {
            if crate::graphics::is_active() {
                crate::graphics::with_framebuffer(|fb| {
                    crate::graphics::cursor::update_cursor_position(
                        fb,
                        last_mouse_x,
                        last_mouse_y,
                        mx,
                        my,
                    );
                });
            }
            last_mouse_x = mx;
            last_mouse_y = my;
        }

        if mouse_state.left_clicked {
            let mut dt = DESKTOP.lock();
            dt.handle_mouse_click(mx, my);
        }

        // 3. Poll for keyboard input (non-blocking)
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
                            AppId::Network => {
                                dt.network.handle_key(key);
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
        } else {
            // Check if audio is actively playing: re-render ONLY the progress bar when seconds tick
            let (is_audio_playing, cur_elapsed, cur_state) = {
                let audio = crate::audio::AUDIO_ENGINE.lock();
                (audio.state == crate::audio::PlaybackState::Playing, audio.elapsed_seconds(), audio.state)
            };

            if is_audio_playing {
                let mut dt = DESKTOP.lock();
                if dt.is_window_open && dt.active_app == AppId::Music {
                    if cur_elapsed != dt.last_music_elapsed || cur_state != dt.last_music_state {
                        dt.last_music_elapsed = cur_elapsed;
                        dt.last_music_state = cur_state;
                        dt.render_music_progress();
                    }
                }
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
