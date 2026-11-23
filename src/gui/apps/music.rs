//! Spotify-Inspired Music Player Application for Desktop GUI.
//!
//! Provides genuine audio playback control, live progress bar, volume adjustment,
//! playlist management, and truthful status indicators.

use crate::audio::{AUDIO_ENGINE, PlaybackState};
use crate::audio::melody::PLAYLIST;
use crate::console::vga::Color;
use crate::gui::compositor::{glyphs, Compositor};
use crate::gui::window::Window;

pub struct MusicApp;

impl MusicApp {
    pub const fn new() -> Self {
        Self
    }

    /// Handles user input directed to the Music Player.
    pub fn handle_key(&mut self, key: u8) {
        let mut audio = AUDIO_ENGINE.lock();
        match key {
            b'p' | b'P' | b' ' => {
                // Play / Pause toggle
                match audio.state {
                    PlaybackState::Playing => {
                        audio.pause();
                        crate::info!("[MUSIC-KEY] 'P' -> Paused playback: '{}'", audio.current_track().title);
                    }
                    PlaybackState::Paused | PlaybackState::Stopped => {
                        audio.play();
                        crate::info!("[MUSIC-KEY] 'P' -> Started playback: '{}'", audio.current_track().title);
                    }
                }
            }
            b'n' | b'N' => {
                audio.next_track();
                crate::info!("[MUSIC-KEY] 'N' -> Next track: '{}'", audio.current_track().title);
            }
            b'v' | b'V' | b'b' | b'B' => {
                audio.prev_track();
                crate::info!("[MUSIC-KEY] 'V' -> Prev track: '{}'", audio.current_track().title);
            }
            b'r' | b'R' => {
                let rep = audio.toggle_repeat();
                crate::info!("[MUSIC-KEY] 'R' -> Repeat: {}", if rep { "ON" } else { "OFF" });
            }
            b'+' | b'=' => {
                let new_vol = (audio.volume + 10).min(100);
                audio.set_volume(new_vol);
                crate::info!("[MUSIC-KEY] '+' -> Volume: {}%", new_vol);
            }
            b'-' | b'_' => {
                let new_vol = audio.volume.saturating_sub(10);
                audio.set_volume(new_vol);
                crate::info!("[MUSIC-KEY] '-' -> Volume: {}%", new_vol);
            }
            b's' | b'S' => {
                audio.stop();
                crate::info!("[MUSIC-KEY] 'S' -> Stopped playback");
            }
            _ => {
                crate::info!("[MUSIC-KEY] Unhandled key: 0x{:02x} ('{}')", key, key as char);
            }
        }
    }

    /// Renders the Spotify-inspired music player into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        let audio = AUDIO_ENGINE.lock();
        let track = audio.current_track();
        let state = audio.state;
        let percent = audio.progress_percent();
        let volume = audio.volume;
        let is_repeat = audio.is_repeat;
        let driver_name = audio.driver_name();
        let elapsed_sec = audio.elapsed_seconds();
        let total_sec = track.duration_seconds as u32;

        // 1. Album Artwork Box (left side)
        let art_fg = match state {
            PlaybackState::Playing => Color::LightGreen,
            PlaybackState::Paused => Color::Yellow,
            PlaybackState::Stopped => Color::Cyan,
        };

        compositor.draw_box(cx + 2, cy + 1, 14, 7, false, art_fg, Color::Black);
        compositor.draw_string(cx + 4, cy + 2, " ~ ALBUM ~ ", Color::LightCyan, Color::Black);
        compositor.put_char(cx + 7, cy + 3, glyphs::MUSIC_NOTE, art_fg, Color::Black);
        compositor.put_char(cx + 9, cy + 3, glyphs::MUSIC_NOTE, art_fg, Color::Black);
        compositor.draw_string(cx + 4, cy + 4, "  BUBBLEOS ", Color::LightGray, Color::Black);
        compositor.draw_string(cx + 4, cy + 5, "  SESSIONS ", Color::LightGray, Color::Black);

        // 2. Track Metadata & Status (right of album art)
        let meta_x = cx + 18;
        compositor.draw_string(meta_x, cy + 1, "Track:   ", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x + 9, cy + 1, track.title, Color::LightCyan, Color::Black);

        compositor.draw_string(meta_x, cy + 2, "Artist:  ", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x + 9, cy + 2, track.artist, Color::White, Color::Black);

        compositor.draw_string(meta_x, cy + 3, "Album:   ", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x + 9, cy + 3, track.album, Color::LightGray, Color::Black);

        compositor.draw_string(meta_x, cy + 4, "Audio:   ", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x + 9, cy + 4, driver_name, Color::LightGreen, Color::Black);

        // Status Indicator Badge (Strict Truthfulness)
        compositor.draw_string(meta_x, cy + 5, "Status:  ", Color::LightGray, Color::Black);
        match state {
            PlaybackState::Playing => {
                compositor.draw_string(meta_x + 9, cy + 5, "[PLAYING] Audio Active", Color::LightGreen, Color::Black);
            }
            PlaybackState::Paused => {
                compositor.draw_string(meta_x + 9, cy + 5, "[PAUSED]  Playback Halted", Color::Yellow, Color::Black);
            }
            PlaybackState::Stopped => {
                compositor.draw_string(meta_x + 9, cy + 5, "[STOPPED] Ready to Play", Color::LightGray, Color::Black);
            }
        }

        // Time display (MM:SS / MM:SS)
        let mut time_buf = [0u8; 16];
        let time_str = format_time(elapsed_sec, total_sec, &mut time_buf);
        compositor.draw_string(meta_x, cy + 6, "Time:    ", Color::LightGray, Color::Black);
        compositor.draw_string(meta_x + 9, cy + 6, time_str, Color::White, Color::Black);

        // 3. Live Progress Slider Bar
        let slider_w = 48;
        let filled = ((percent as usize * slider_w) / 100).min(slider_w);

        let mut slider_chars = [b'-'; 50];
        for i in 0..filled {
            slider_chars[i] = b'=';
        }
        if filled < slider_w {
            slider_chars[filled] = b'>';
        }
        let slider_str = core::str::from_utf8(&slider_chars[..slider_w]).unwrap_or("");

        let mut elapsed_buf = [0u8; 8];
        let mut total_buf = [0u8; 8];
        let e_str = format_seconds(elapsed_sec, &mut elapsed_buf);
        let t_str = format_seconds(total_sec, &mut total_buf);

        compositor.draw_string(cx + 2, cy + 9, e_str, Color::LightCyan, Color::Black);
        compositor.put_char(cx + 8, cy + 9, b'[', Color::Cyan, Color::Black);
        compositor.draw_string(cx + 9, cy + 9, slider_str, Color::LightCyan, Color::Black);
        compositor.put_char(cx + 9 + slider_w, cy + 9, b']', Color::Cyan, Color::Black);
        compositor.draw_string(cx + 11 + slider_w, cy + 9, t_str, Color::LightGray, Color::Black);

        // 4. Playback Controls Bar
        let play_btn_text = match state {
            PlaybackState::Playing => "[|| Pause (P)]",
            PlaybackState::Paused =>  "[> Resume (P)]",
            PlaybackState::Stopped => "[> Play (P)]  ",
        };
        let play_btn_fg = match state {
            PlaybackState::Playing => Color::LightGreen,
            PlaybackState::Paused => Color::Yellow,
            PlaybackState::Stopped => Color::LightCyan,
        };

        compositor.draw_string(cx + 2, cy + 11, "[<< Prev (V)]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 17, cy + 11, play_btn_text, play_btn_fg, Color::Black);
        compositor.draw_string(cx + 33, cy + 11, "[>> Next (N)]", Color::LightCyan, Color::Black);

        let rep_str = if is_repeat { "[Repeat: ON (R)]" } else { "[Repeat: OFF (R)]" };
        let rep_fg = if is_repeat { Color::LightGreen } else { Color::LightGray };
        compositor.draw_string(cx + 48, cy + 11, rep_str, rep_fg, Color::Black);

        // Volume Indicator
        let mut vol_buf = [0u8; 16];
        let vol_str = format_volume(volume, &mut vol_buf);
        compositor.draw_string(cx + 48, cy + 12, vol_str, Color::LightCyan, Color::Black);

        // 5. Playlist Box
        compositor.draw_box(cx + 2, cy + 13, 62, 8, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 4, cy + 13, "[ Playlist: Featured Song & Chiptune Scores ]", Color::LightCyan, Color::Black);

        for (i, p_track) in PLAYLIST.iter().enumerate() {
            let row = cy + 15 + i;
            let is_curr = i == audio.current_track_idx;
            let marker = if is_curr { ">" } else { " " };
            let fg = if is_curr {
                match state {
                    PlaybackState::Playing => Color::LightGreen,
                    PlaybackState::Paused => Color::Yellow,
                    PlaybackState::Stopped => Color::LightCyan,
                }
            } else {
                Color::White
            };

            let status_badge = if is_curr {
                match state {
                    PlaybackState::Playing => " [PLAYING]",
                    PlaybackState::Paused =>  " [PAUSED] ",
                    PlaybackState::Stopped => " [STOPPED]",
                }
            } else {
                ""
            };

            let mut track_line = [b' '; 56];
            let prefix = format_track_line(i + 1, marker, p_track.title, p_track.artist, p_track.duration_seconds, status_badge, &mut track_line);
            compositor.draw_string(cx + 4, row, prefix, fg, Color::Black);
        }

        compositor.draw_string(cx + 4, cy + 19, "Hotkeys: [P/Space] Play/Pause | [N] Next | [V] Prev | [+/-] Vol | [R] Rep", Color::LightGray, Color::Black);
    }

    /// Renders the Spotify-inspired pixel-based music player into the window client area.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let ch = Window::CLIENT_PIXEL_HEIGHT;

        // Dark Spotify-like background surface (#161022)
        let music_bg = Color::rgb(22, 16, 34);
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 4, music_bg);
        draw::draw_rounded_rect(fb, cx, cy, cw, ch, 4, Color::rgb(55, 40, 75));

        let audio = AUDIO_ENGINE.lock();
        let track = audio.current_track();
        let state = audio.state;
        let percent = audio.progress_percent();
        let volume = audio.volume;
        let is_repeat = audio.is_repeat;
        let driver_name = audio.driver_name();
        let elapsed_sec = audio.elapsed_seconds();
        let total_sec = track.duration_seconds as u32;

        // 1. Album Artwork Tile (x = cx + 24, y = cy + 24, w = 180, h = 180)
        let art_x = cx + 24;
        let art_y = cy + 24;
        let art_w = 180;
        let art_h = 180;

        draw::draw_shadow(fb, art_x, art_y, art_w, art_h, 8);
        let art_bg = match state {
            PlaybackState::Playing => Color::rgb(70, 35, 110),
            PlaybackState::Paused => Color::rgb(80, 60, 40),
            PlaybackState::Stopped => Color::rgb(45, 35, 60),
        };
        draw::fill_rounded_rect(fb, art_x, art_y, art_w, art_h, 8, art_bg);
        draw::draw_rounded_rect(fb, art_x, art_y, art_w, art_h, 8, theme::PRIMARY_PURPLE);

        // Concentric Vinyl Record Rings inside Artwork
        let center_x = (art_x + art_w / 2) as isize;
        let center_y = (art_y + art_h / 2) as isize;
        for ring_offset in (10..60).step_by(10) {
            draw::draw_rounded_rect(
                fb,
                (center_x - ring_offset) as usize,
                (center_y - ring_offset) as usize,
                (ring_offset * 2) as usize,
                (ring_offset * 2) as usize,
                ring_offset as usize,
                Color::rgb(105, 65, 155),
            );
        }
        // Center Spindle
        draw::fill_rounded_rect(fb, (center_x - 14) as usize, (center_y - 14) as usize, 28, 28, 14, Color::rgb(220, 180, 255));
        draw::fill_rounded_rect(fb, (center_x - 6) as usize, (center_y - 6) as usize, 12, 12, 6, Color::rgb(30, 20, 45));

        font::draw_string(fb, art_x + 50, art_y + 154, "MAYIIIII", theme::WHITE, art_bg);

        // 2. Track Metadata & Status Badges (x = art_x + art_w + 32)
        let meta_x = art_x + art_w + 32;
        let meta_y = cy + 28;

        font::draw_string(fb, meta_x, meta_y, "CURRENTLY PLAYING", Color::rgb(160, 145, 185), music_bg);
        font::draw_string_scaled(fb, meta_x, meta_y + 20, track.title, theme::WHITE, 2);

        font::draw_string(fb, meta_x, meta_y + 60, "Artist:  ", Color::rgb(160, 145, 185), music_bg);
        font::draw_string(fb, meta_x + 80, meta_y + 60, track.artist, Color::rgb(220, 205, 245), music_bg);

        font::draw_string(fb, meta_x, meta_y + 84, "Album:   ", Color::rgb(160, 145, 185), music_bg);
        font::draw_string(fb, meta_x + 80, meta_y + 84, track.album, Color::rgb(200, 185, 230), music_bg);

        font::draw_string(fb, meta_x, meta_y + 108, "Audio:   ", Color::rgb(160, 145, 185), music_bg);
        font::draw_string(fb, meta_x + 80, meta_y + 108, driver_name, Color::rgb(100, 220, 140), music_bg);

        // Status Badge Pill
        let badge_y = meta_y + 136;
        match state {
            PlaybackState::Playing => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(35, 120, 65));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "> PLAYING", theme::WHITE, Color::rgb(35, 120, 65));
            }
            PlaybackState::Paused => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(150, 110, 25));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "|| PAUSED", theme::WHITE, Color::rgb(150, 110, 25));
            }
            PlaybackState::Stopped => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(70, 60, 85));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "[] STOPPED", theme::WHITE, Color::rgb(70, 60, 85));
            }
        }

        // Format times
        let mut time_buf = [0u8; 16];
        let time_str = format_time(elapsed_sec, total_sec, &mut time_buf);
        font::draw_string(fb, meta_x + 160, badge_y + 6, time_str, Color::rgb(220, 210, 240), music_bg);

        // 3. Playlist Card (x = cx + 24, y = cy + 224, w = cw - 48, h = 180)
        let pl_y = cy + 224;
        let pl_w = cw - 48;
        let pl_h = 180;
        draw::fill_rounded_rect(fb, cx + 24, pl_y, pl_w, pl_h, 6, Color::rgb(30, 22, 45));
        draw::draw_rounded_rect(fb, cx + 24, pl_y, pl_w, pl_h, 6, Color::rgb(65, 48, 88));

        font::draw_string(fb, cx + 40, pl_y + 12, "PLAYLIST  |  FEATURED AUDIO TRACK & CHIPTUNE SCORES", theme::PRIMARY_PURPLE, Color::rgb(30, 22, 45));
        draw::draw_hline(fb, cx + 24, cx + 24 + pl_w, pl_y + 34, Color::rgb(65, 48, 88));

        for (i, p_track) in PLAYLIST.iter().enumerate() {
            let row_y = pl_y + 44 + i * 38;
            let is_curr = i == audio.current_track_idx;
            if is_curr {
                draw::fill_rounded_rect(fb, cx + 32, row_y - 4, pl_w - 16, 32, 4, Color::rgb(55, 38, 80));
                draw::fill_rounded_rect(fb, cx + 32, row_y - 2, 4, 28, 2, theme::PRIMARY_PURPLE);
            }

            let marker = if is_curr { "> " } else { "  " };
            let fg = if is_curr {
                match state {
                    PlaybackState::Playing => Color::rgb(100, 230, 140),
                    PlaybackState::Paused => Color::rgb(255, 210, 80),
                    PlaybackState::Stopped => Color::rgb(180, 160, 220),
                }
            } else {
                Color::rgb(200, 190, 220)
            };

            let status_badge = if is_curr {
                match state {
                    PlaybackState::Playing => " [PLAYING]",
                    PlaybackState::Paused =>  " [PAUSED] ",
                    PlaybackState::Stopped => " [STOPPED]",
                }
            } else {
                ""
            };

            let mut track_line = [b' '; 56];
            let prefix = format_track_line(i + 1, marker, p_track.title, p_track.artist, p_track.duration_seconds, status_badge, &mut track_line);
            font::draw_string(fb, cx + 44, row_y + 4, prefix, fg, if is_curr { Color::rgb(55, 38, 80) } else { Color::rgb(30, 22, 45) });
        }

        // 4. Player Controls, Live Progress Bar & Sliders
        let ctrl_y = cy + 424;

        // Live Progress Bar (Full Width)
        let prog_x = cx + 32;
        let prog_w = cw - 64;
        let prog_h = 10;
        let bar_y = ctrl_y + 16;

        let mut elapsed_buf = [0u8; 8];
        let mut total_buf = [0u8; 8];
        let e_str = format_seconds(elapsed_sec, &mut elapsed_buf);
        let t_str = format_seconds(total_sec, &mut total_buf);

        font::draw_string(fb, prog_x, bar_y - 2, e_str, Color::rgb(160, 145, 185), music_bg);
        font::draw_string(fb, prog_x + prog_w - 40, bar_y - 2, t_str, Color::rgb(160, 145, 185), music_bg);

        let actual_bar_x = prog_x + 50;
        let actual_bar_w = prog_w.saturating_sub(100);

        // Progress bar background track
        draw::fill_rounded_rect(fb, actual_bar_x, bar_y + 2, actual_bar_w, prog_h, 5, Color::rgb(45, 35, 65));

        // Filled progress bar
        let filled_px = ((percent as usize * actual_bar_w) / 100).min(actual_bar_w);
        if filled_px > 0 {
            let fill_color = match state {
                PlaybackState::Playing => Color::rgb(130, 85, 210),
                PlaybackState::Paused => Color::rgb(180, 130, 50),
                PlaybackState::Stopped => Color::rgb(90, 75, 115),
            };
            draw::fill_rounded_rect(fb, actual_bar_x, bar_y + 2, filled_px, prog_h, 5, fill_color);
            // Thumb knob
            let thumb_x = actual_bar_x + filled_px;
            draw::fill_rounded_rect(fb, thumb_x.saturating_sub(6), bar_y - 1, 12, 16, 6, theme::WHITE);
        }

        // Playback Control Buttons (Centered)
        let btn_row_y = bar_y + 36;
        let mid_x = cx + cw / 2;

        draw::draw_button(fb, mid_x - 180, btn_row_y, 100, 36, "<< Prev", false, false);

        let (play_label, play_primary) = match state {
            PlaybackState::Playing => ("|| Pause", true),
            PlaybackState::Paused =>  ("> Resume", true),
            PlaybackState::Stopped => ("> Play", true),
        };
        draw::draw_button(fb, mid_x - 60, btn_row_y - 3, 120, 42, play_label, true, play_primary);

        draw::draw_button(fb, mid_x + 80, btn_row_y, 100, 36, "Next >>", false, false);

        let rep_str = if is_repeat { "Repeat: ON" } else { "Repeat: OFF" };
        draw::draw_button(fb, mid_x + 200, btn_row_y, 120, 36, rep_str, false, is_repeat);

        // Volume Bar
        let vol_y = btn_row_y + 58;
        let vol_x = mid_x - 150;
        font::draw_string(fb, vol_x - 60, vol_y + 2, "Vol:", Color::rgb(160, 145, 185), music_bg);
        draw::fill_rounded_rect(fb, vol_x, vol_y + 4, 200, 8, 4, Color::rgb(45, 35, 65));
        let vol_fill = ((volume as usize * 200) / 100).min(200);
        if vol_fill > 0 {
            draw::fill_rounded_rect(fb, vol_x, vol_y + 4, vol_fill, 8, 4, theme::PRIMARY_PURPLE);
        }
        let mut vol_buf = [0u8; 16];
        let vol_str = format_volume(volume, &mut vol_buf);
        font::draw_string(fb, vol_x + 215, vol_y + 2, vol_str, Color::rgb(220, 210, 240), music_bg);

        // Bottom Hotkey Instructions
        font::draw_string(fb, cx + 40, cy + ch - 28, "Controls: [P/Space] Play/Pause | [N] Next | [V] Prev | [+/-] Volume | [R] Repeat | [Esc] Minimize", Color::rgb(140, 125, 165), music_bg);
    }

    /// Efficiently redraws only the live progress slider, timestamp, and status badge
    /// without re-rendering the full window or clearing the screen.
    pub fn draw_pixel_progress(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let music_bg = Color::rgb(22, 16, 34);

        let audio = AUDIO_ENGINE.lock();
        let track = audio.current_track();
        let state = audio.state;
        let percent = audio.progress_percent();
        let elapsed_sec = audio.elapsed_seconds();
        let total_sec = track.duration_seconds as u32;
        drop(audio);

        // 1. Status Indicator Badge & Time (cy + 164)
        let meta_x = cx + 224;
        let badge_y = cy + 164;
        fb.fill_rect(meta_x, badge_y, 350, 28, music_bg);

        match state {
            PlaybackState::Playing => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(35, 140, 65));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "> PLAYING", theme::WHITE, Color::rgb(35, 140, 65));
            }
            PlaybackState::Paused => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(150, 110, 25));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "|| PAUSED", theme::WHITE, Color::rgb(150, 110, 25));
            }
            PlaybackState::Stopped => {
                draw::fill_rounded_rect(fb, meta_x, badge_y, 140, 28, 14, Color::rgb(70, 60, 85));
                font::draw_string(fb, meta_x + 16, badge_y + 6, "[] STOPPED", theme::WHITE, Color::rgb(70, 60, 85));
            }
        }

        let mut time_buf = [0u8; 16];
        let time_str = format_time(elapsed_sec, total_sec, &mut time_buf);
        font::draw_string(fb, meta_x + 160, badge_y + 6, time_str, Color::rgb(220, 210, 240), music_bg);

        // 2. Live Progress Bar (ctrl_y + 16 = cy + 440)
        let ctrl_y = cy + 424;
        let prog_x = cx + 32;
        let prog_w = cw - 64;
        let prog_h = 10;
        let bar_y = ctrl_y + 16;

        fb.fill_rect(prog_x, bar_y - 4, prog_w, 24, music_bg);

        let mut elapsed_buf = [0u8; 8];
        let mut total_buf = [0u8; 8];
        let e_str = format_seconds(elapsed_sec, &mut elapsed_buf);
        let t_str = format_seconds(total_sec, &mut total_buf);

        font::draw_string(fb, prog_x, bar_y - 2, e_str, Color::rgb(160, 145, 185), music_bg);
        font::draw_string(fb, prog_x + prog_w - 40, bar_y - 2, t_str, Color::rgb(160, 145, 185), music_bg);

        let actual_bar_x = prog_x + 50;
        let actual_bar_w = prog_w.saturating_sub(100);

        // Progress bar background track
        draw::fill_rounded_rect(fb, actual_bar_x, bar_y + 2, actual_bar_w, prog_h, 5, Color::rgb(45, 35, 65));

        // Filled progress bar
        let filled_px = ((percent as usize * actual_bar_w) / 100).min(actual_bar_w);
        if filled_px > 0 {
            let fill_color = match state {
                PlaybackState::Playing => Color::rgb(130, 85, 210),
                PlaybackState::Paused => Color::rgb(180, 130, 50),
                PlaybackState::Stopped => Color::rgb(90, 75, 115),
            };
            draw::fill_rounded_rect(fb, actual_bar_x, bar_y + 2, filled_px, prog_h, 5, fill_color);
            // Thumb knob
            let thumb_x = actual_bar_x + filled_px;
            draw::fill_rounded_rect(fb, thumb_x.saturating_sub(6), bar_y - 1, 12, 16, 6, theme::WHITE);
        }
    }
}

fn format_seconds<'a>(sec: u32, buf: &'a mut [u8; 8]) -> &'a str {
    let m = (sec / 60) % 60;
    let s = sec % 60;
    buf[0] = b'0' + (m / 10) as u8;
    buf[1] = b'0' + (m % 10) as u8;
    buf[2] = b':';
    buf[3] = b'0' + (s / 10) as u8;
    buf[4] = b'0' + (s % 10) as u8;
    core::str::from_utf8(&buf[..5]).unwrap_or("00:00")
}

fn format_time<'a>(elapsed: u32, total: u32, buf: &'a mut [u8; 16]) -> &'a str {
    let em = (elapsed / 60) % 60;
    let es = elapsed % 60;
    let tm = (total / 60) % 60;
    let ts = total % 60;

    buf[0] = b'0' + (em / 10) as u8;
    buf[1] = b'0' + (em % 10) as u8;
    buf[2] = b':';
    buf[3] = b'0' + (es / 10) as u8;
    buf[4] = b'0' + (es % 10) as u8;
    buf[5] = b' ';
    buf[6] = b'/';
    buf[7] = b' ';
    buf[8] = b'0' + (tm / 10) as u8;
    buf[9] = b'0' + (tm % 10) as u8;
    buf[10] = b':';
    buf[11] = b'0' + (ts / 10) as u8;
    buf[12] = b'0' + (ts % 10) as u8;

    core::str::from_utf8(&buf[..13]).unwrap_or("00:00 / 00:00")
}

fn format_volume<'a>(vol: u8, buf: &'a mut [u8; 16]) -> &'a str {
    let prefix = b"Vol: ";
    buf[..5].copy_from_slice(prefix);
    let mut len = 5;
    if vol >= 100 {
        buf[len] = b'1';
        buf[len+1] = b'0';
        buf[len+2] = b'0';
        len += 3;
    } else {
        buf[len] = b'0' + (vol / 10);
        buf[len+1] = b'0' + (vol % 10);
        len += 2;
    }
    buf[len] = b'%';
    len += 1;
    core::str::from_utf8(&buf[..len]).unwrap_or("Vol: 80%")
}

fn format_track_line<'a>(
    num: usize,
    marker: &str,
    title: &str,
    artist: &str,
    dur: u16,
    badge: &str,
    buf: &'a mut [u8; 56],
) -> &'a str {
    buf[0] = b'0' + (num as u8);
    buf[1] = b'.';
    buf[2] = b' ';
    buf[3] = marker.as_bytes()[0];
    buf[4] = b' ';
    let mut idx = 5;

    for &b in title.as_bytes() {
        if idx >= 24 { break; }
        buf[idx] = b;
        idx += 1;
    }
    while idx < 24 {
        buf[idx] = b' ';
        idx += 1;
    }

    buf[idx] = b'-';
    buf[idx+1] = b' ';
    idx += 2;

    for &b in artist.as_bytes() {
        if idx >= 40 { break; }
        buf[idx] = b;
        idx += 1;
    }
    while idx < 40 {
        buf[idx] = b' ';
        idx += 1;
    }

    // Duration (00:0X)
    let s = dur % 60;
    buf[idx] = b'(';
    buf[idx+1] = b'0';
    buf[idx+2] = b'0';
    buf[idx+3] = b':';
    buf[idx+4] = b'0' + (s / 10) as u8;
    buf[idx+5] = b'0' + (s % 10) as u8;
    buf[idx+6] = b')';
    idx += 7;

    for &b in badge.as_bytes() {
        if idx >= 56 { break; }
        buf[idx] = b;
        idx += 1;
    }

    core::str::from_utf8(&buf[..idx]).unwrap_or("")
}
