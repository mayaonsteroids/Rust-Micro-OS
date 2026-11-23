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
        compositor.draw_string(cx + 4, cy + 4, "  MICRO-OS ", Color::LightGray, Color::Black);
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
