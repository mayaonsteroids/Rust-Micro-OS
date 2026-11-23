//! Music Player Application (Spotify-Inspired) for Desktop GUI.
//!
//! Provides track metadata, album art preview, progress slider,
//! playback controls, volume slider, and interactive playlist.

use crate::console::vga::Color;
use crate::gui::compositor::{glyphs, Compositor};
use crate::gui::window::Window;

pub struct MusicApp;

impl MusicApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders the Spotify-inspired music player into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // 1. Album Artwork Box (left side)
        compositor.draw_box(cx + 2, cy + 1, 14, 7, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 2, " ~ ALBUM ~ ", Color::LightCyan, Color::Black);
        compositor.put_char(cx + 7, cy + 3, glyphs::MUSIC_NOTE, Color::LightCyan, Color::Black);
        compositor.put_char(cx + 9, cy + 3, glyphs::MUSIC_NOTE, Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 4, "  LAVENDER ", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 4, cy + 5, "   DREAMS  ", Color::LightGray, Color::Black);

        // 2. Track Metadata (right of album art)
        let meta_x = cx + 18;
        compositor.draw_string(meta_x, cy + 1, "Track:   Lavender Dreams", Color::LightCyan, Color::Black);
        compositor.draw_string(meta_x, cy + 2, "Artist:  Maya Rust", Color::White, Color::Black);
        compositor.draw_string(meta_x, cy + 3, "Album:   Bare Metal Volume 1 (M17)", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x, cy + 4, "Format:  16-bit 44.1 kHz PCM Audio (Stereo)", Color::White, Color::Black);
        compositor.draw_string(meta_x, cy + 5, "Device:  Intel ICH AC97 (00:05.0) + PC Speaker", Color::LightGreen, Color::Black);
        compositor.draw_string(meta_x, cy + 6, "Time:    01:42 / 03:20 (51% Complete)", Color::White, Color::Black);

        // 3. Progress Slider
        compositor.draw_string(cx + 2, cy + 9, "01:42 ", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 8, cy + 9, "[===========================>-----------------------]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 62, cy + 9, " 03:20", Color::LightGray, Color::Black);

        // 4. Playback Controls
        compositor.draw_string(
            cx + 4,
            cy + 11,
            "[<< Prev]   [> Play]   [|| Pause]   [>> Next]   [Repeat]   Vol: 80%",
            Color::LightCyan,
            Color::Black,
        );

        // 5. Playlist Box
        compositor.draw_box(cx + 2, cy + 13, 62, 8, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 13, "[ Playlist: Featured Tracks ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 15, "1. > Lavender Dreams - Maya Rust (03:20) [PLAYING]", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 4, cy + 16, "2.   Kernel Panic Blues - The Interrupts (02:45)", Color::White, Color::Black);
        compositor.draw_string(cx + 4, cy + 17, "3.   DMA Symphony in C - 82540EM Gigabit (04:12)", Color::White, Color::Black);
        compositor.draw_string(cx + 4, cy + 18, "4.   Chiptune Memory - PIT Channel 2 (01:58)", Color::White, Color::Black);
    }
}
