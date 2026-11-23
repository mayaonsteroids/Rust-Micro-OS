//! Unified Audio Subsystem for Rust Micro-OS.
//!
//! Provides two-tier audio architecture:
//! - Tier 1: Intel ICH AC97 PCI DMA streaming (VirtualBox / QEMU).
//! - Tier 2: Programmable PC Speaker tone generator fallback (Universal).

pub mod ac97;
pub mod melody;
pub mod speaker;

use spin::Mutex;
use crate::audio::ac97::Ac97Device;
use crate::audio::melody::{Track, PLAYLIST};
use crate::info;

/// Playback state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioEngine {
    pub state: PlaybackState,
    pub current_track_idx: usize,
    pub note_idx: usize,
    pub ticks_in_note: u16,
    pub elapsed_ticks: u32,
    pub is_repeat: bool,
    pub volume: u8,
    pub ac97: Option<Ac97Device>,
}

impl AudioEngine {
    pub const fn new() -> Self {
        Self {
            state: PlaybackState::Stopped,
            current_track_idx: 0,
            note_idx: 0,
            ticks_in_note: 0,
            elapsed_ticks: 0,
            is_repeat: true,
            volume: 80,
            ac97: None,
        }
    }

    /// Initializes audio hardware (detecting AC97 or falling back to PC Speaker).
    pub fn init(&mut self) {
        info!("========================================================");
        info!("             INITIALIZING AUDIO SUBSYSTEM               ");
        info!("========================================================");

        // Attempt Intel ICH AC97 initialization
        self.ac97 = Ac97Device::init();

        if self.ac97.is_some() {
            info!("[AUDIO] Primary Driver Active: Intel ICH AC97 DMA + PC Speaker");
        } else {
            info!("[AUDIO] Primary Driver Active: PC Speaker Synthesizer (PIT Ch2 0x42/0x61)");
        }

        self.set_volume(80);
    }

    /// Returns the currently loaded track.
    pub fn current_track(&self) -> &'static Track {
        &PLAYLIST[self.current_track_idx % PLAYLIST.len()]
    }

    /// Starts or resumes audio playback.
    pub fn play(&mut self) {
        let track = self.current_track();

        match self.state {
            PlaybackState::Playing => return,
            PlaybackState::Paused => {
                // Resume from paused position
                self.state = PlaybackState::Playing;
                if let Some(ref mut dev) = self.ac97 {
                    dev.start();
                }
                // Play current note
                if self.note_idx < track.notes.len() {
                    let note = track.notes[self.note_idx];
                    speaker::play_tone(note.freq_hz);
                }
                info!("[AUDIO] Resumed playback: '{}' at note {} (elapsed: {} ticks)",
                    track.title, self.note_idx, self.elapsed_ticks);
            }
            PlaybackState::Stopped => {
                // Start from beginning
                self.state = PlaybackState::Playing;
                self.note_idx = 0;
                self.ticks_in_note = 0;
                self.elapsed_ticks = 0;

                if let Some(ref mut dev) = self.ac97 {
                    dev.start();
                }
                if let Some(note) = track.notes.first() {
                    speaker::play_tone(note.freq_hz);
                }
                info!("[AUDIO] Started playback: '{}' by {} ({} notes, {}s)",
                    track.title, track.artist, track.notes.len(), track.duration_seconds);
            }
        }
    }

    /// Pauses audio playback without losing playback position.
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
            if let Some(ref mut dev) = self.ac97 {
                dev.pause();
            }
            speaker::silence();
            info!("[AUDIO] Paused playback: '{}' at note {}",
                self.current_track().title, self.note_idx);
        }
    }

    /// Stops audio playback and resets position to the beginning.
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        if let Some(ref mut dev) = self.ac97 {
            dev.pause();
        }
        speaker::silence();
        self.note_idx = 0;
        self.ticks_in_note = 0;
        self.elapsed_ticks = 0;
        info!("[AUDIO] Stopped audio playback.");
    }

    /// Switches to the next track in the playlist.
    pub fn next_track(&mut self) {
        let was_playing = self.state == PlaybackState::Playing;
        self.current_track_idx = (self.current_track_idx + 1) % PLAYLIST.len();
        self.note_idx = 0;
        self.ticks_in_note = 0;
        self.elapsed_ticks = 0;

        info!("[AUDIO] Switched to next track: '{}'", self.current_track().title);

        if was_playing {
            self.state = PlaybackState::Stopped;
            self.play();
        } else {
            speaker::silence();
            if let Some(ref mut dev) = self.ac97 {
                dev.pause();
            }
        }
    }

    /// Switches to the previous track in the playlist.
    pub fn prev_track(&mut self) {
        let was_playing = self.state == PlaybackState::Playing;
        self.current_track_idx = if self.current_track_idx == 0 {
            PLAYLIST.len() - 1
        } else {
            self.current_track_idx - 1
        };
        self.note_idx = 0;
        self.ticks_in_note = 0;
        self.elapsed_ticks = 0;

        info!("[AUDIO] Switched to previous track: '{}'", self.current_track().title);

        if was_playing {
            self.state = PlaybackState::Stopped;
            self.play();
        } else {
            speaker::silence();
            if let Some(ref mut dev) = self.ac97 {
                dev.pause();
            }
        }
    }

    /// Toggles repeat mode.
    pub fn toggle_repeat(&mut self) -> bool {
        self.is_repeat = !self.is_repeat;
        info!("[AUDIO] Repeat mode set to: {}", self.is_repeat);
        self.is_repeat
    }

    /// Sets playback volume (0..100).
    pub fn set_volume(&mut self, vol: u8) {
        self.volume = vol.min(100);
        if let Some(ref mut dev) = self.ac97 {
            dev.set_volume(self.volume);
        }
        info!("[AUDIO] Volume set to: {}%", self.volume);
    }

    /// Advances playback position by 1 timer tick (called from event loop or PIT timer).
    pub fn tick(&mut self) {
        if self.state != PlaybackState::Playing {
            return;
        }

        let track = self.current_track();
        self.elapsed_ticks += 1;
        self.ticks_in_note += 1;

        if self.note_idx < track.notes.len() {
            let current_note = track.notes[self.note_idx];
            if self.ticks_in_note >= current_note.duration_ticks {
                // Advance to next note
                self.note_idx += 1;
                self.ticks_in_note = 0;

                if self.note_idx >= track.notes.len() {
                    // Track finished
                    if self.is_repeat {
                        self.note_idx = 0;
                        self.elapsed_ticks = 0;
                        if let Some(first_note) = track.notes.first() {
                            speaker::play_tone(first_note.freq_hz);
                        }
                    } else {
                        self.next_track();
                    }
                } else {
                    let next_note = track.notes[self.note_idx];
                    speaker::play_tone(next_note.freq_hz);
                }
            }
        } else {
            self.stop();
        }
    }

    /// Returns human-readable driver status string.
    pub fn driver_name(&self) -> &'static str {
        if self.ac97.is_some() {
            "Intel ICH AC97 (00:05.0) + PC Speaker"
        } else {
            "PC Speaker Synthesizer (Universal)"
        }
    }

    /// Returns the progress percentage (0..100).
    pub fn progress_percent(&self) -> u8 {
        let total = self.current_track().total_ticks();
        if total > 0 {
            ((self.elapsed_ticks as u64 * 100) / total as u64).min(100) as u8
        } else {
            0
        }
    }
}

/// Global thread-safe audio engine instance.
pub static AUDIO_ENGINE: Mutex<AudioEngine> = Mutex::new(AudioEngine::new());

/// Initializes the global audio engine.
pub fn init() {
    AUDIO_ENGINE.lock().init();
}

/// Periodic audio tick to advance notes and streams.
pub fn tick() {
    AUDIO_ENGINE.lock().tick();
}
