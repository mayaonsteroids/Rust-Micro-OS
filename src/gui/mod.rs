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
use crate::gui::compositor::Compositor;
use crate::gui::dock::AppId;
use crate::gui::window::Window;

/// Global off-screen compositor buffer allocated in BSS (bypasses 256 KiB heap).
static COMPOSITOR: Mutex<Compositor> = Mutex::new(Compositor::new());

/// Desktop Environment State
pub struct Desktop {
    pub active_app: AppId,
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

        // 1. Draw Left Application Dock
        dock::draw_dock(&mut comp, self.active_app);

        // 2. Draw Active Window Frame
        let window = Window::standard(self.active_app.title());
        window.draw_frame(&mut comp, true);

        // 3. Render Active Application Content
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

        // 4. Atomic Present to VGA Text Buffer 0xb8000
        comp.present();
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

    loop {
        // 1. Advance audio playback tick
        crate::audio::tick();

        // 2. Poll for keyboard input (non-blocking)
        if let Some(key) = crate::console::try_read_byte() {
            let mut dt = DESKTOP.lock();

            match key {
                b'\x1b' => {
                    // Esc: Exit desktop mode
                    crate::info!("[DESKTOP] Exited window manager via Escape key.");
                    crate::audio::AUDIO_ENGINE.lock().pause();
                    break;
                }
                b'\t' => {
                    // Tab: Switch to next application
                    dt.active_app = dt.active_app.next();
                    crate::info!("[DESKTOP] Switched active application to: {}", dt.active_app.title());
                }
                b'1'..=b'8' => {
                    // Quick-switch via number keys 1..8
                    let target_app = AppId::from_index(key - b'1');
                    if dt.active_app == AppId::Terminal && key >= b'1' && key <= b'8' && dt.terminal.input_len > 0 {
                        // If user is actively typing a command in Terminal, forward the digit
                        dt.terminal.handle_key(key);
                    } else {
                        dt.active_app = target_app;
                        crate::info!("[DESKTOP] Switched active application to: {}", dt.active_app.title());
                    }
                }
                _ => {
                    // Forward input to active app
                    match dt.active_app {
                        AppId::Terminal => {
                            dt.terminal.handle_key(key);
                            if dt.terminal.should_exit_desktop {
                                dt.terminal.should_exit_desktop = false;
                                crate::info!("[DESKTOP] Exited window manager via Terminal exit command.");
                                crate::audio::AUDIO_ENGINE.lock().pause();
                                break;
                            }
                        }
                        AppId::Music => {
                            dt.music.handle_key(key);
                        }
                        _ => {}
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

            // Sleep CPU until next hardware interrupt (PIT timer tick @ 100 Hz or keyboard IRQ)
            x86_64::instructions::hlt();
        }
    }

    // Restore standard CLI VGA screen
    vga::clear_screen();
    vga::set_color(Color::White, Color::Black);
    crate::info!("[DESKTOP] Returned to interactive CLI shell.");
}
