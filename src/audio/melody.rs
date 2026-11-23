//! Original Chiptune Compositions and Track Metadata.
//!
//! Provides non-copyrighted, mathematically composed musical scores
//! for testing audio playback, tone generation, and playlist navigation.

/// Represents a single musical note with pitch (Hz) and duration (100 Hz ticks).
#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub freq_hz: u16,        // Frequency in Hz (0 = rest/silence)
    pub duration_ticks: u16, // Duration in 100 Hz timer ticks (10 ticks = 100 ms)
}

impl Note {
    pub const fn tone(freq_hz: u16, ticks: u16) -> Self {
        Self { freq_hz, duration_ticks: ticks }
    }

    pub const fn rest(ticks: u16) -> Self {
        Self { freq_hz: 0, duration_ticks: ticks }
    }
}

// Standard Musical Note Frequencies (Hz)
pub mod notes {
    pub const REST: u16 = 0;
    pub const C3: u16 = 131;
    pub const D3: u16 = 147;
    pub const E3: u16 = 165;
    pub const F3: u16 = 175;
    pub const G3: u16 = 196;
    pub const A3: u16 = 220;
    pub const B3: u16 = 247;

    pub const C4: u16 = 262;
    pub const D4: u16 = 294;
    pub const E4: u16 = 330;
    pub const F4: u16 = 349;
    pub const G4: u16 = 392;
    pub const A4: u16 = 440;
    pub const B4: u16 = 494;

    pub const C5: u16 = 523;
    pub const D5: u16 = 587;
    pub const E5: u16 = 659;
    pub const F5: u16 = 698;
    pub const G5: u16 = 784;
    pub const A5: u16 = 880;
    pub const B5: u16 = 988;

    pub const C6: u16 = 1047;
}

use notes::*;

/// Track 1: "Lavender Horizon" — Original upbeat melodic arpeggios in C Major.
pub static TRACK_1_NOTES: [Note; 32] = [
    Note::tone(C4, 15), Note::tone(E4, 15), Note::tone(G4, 15), Note::tone(C5, 25),
    Note::tone(B4, 15), Note::tone(G4, 15), Note::tone(A4, 25), Note::rest(10),
    Note::tone(F4, 15), Note::tone(A4, 15), Note::tone(C5, 20), Note::tone(B4, 15),
    Note::tone(G4, 25), Note::tone(E4, 15), Note::tone(C4, 30), Note::rest(15),

    Note::tone(G4, 15), Note::tone(E4, 15), Note::tone(G4, 15), Note::tone(C5, 25),
    Note::tone(D5, 15), Note::tone(C5, 15), Note::tone(B4, 20), Note::tone(A4, 15),
    Note::tone(G4, 20), Note::tone(F4, 15), Note::tone(E4, 20), Note::tone(D4, 20),
    Note::tone(C4, 45), Note::rest(20), Note::tone(G4, 20), Note::tone(C5, 50),
];

/// Track 2: "Kernel Pulse" — Original rhythmic electronic theme in D Minor.
pub static TRACK_2_NOTES: [Note; 28] = [
    Note::tone(D3, 12), Note::tone(D3, 12), Note::tone(F3, 15), Note::tone(G3, 15),
    Note::tone(A3, 25), Note::tone(F3, 15), Note::tone(D3, 30), Note::rest(10),

    Note::tone(D3, 12), Note::tone(D3, 12), Note::tone(A3, 15), Note::tone(C4, 15),
    Note::tone(D4, 30), Note::tone(C4, 15), Note::tone(A3, 25), Note::rest(15),

    Note::tone(F4, 15), Note::tone(E4, 15), Note::tone(D4, 20), Note::tone(A3, 20),
    Note::tone(C4, 20), Note::tone(A3, 15), Note::tone(G3, 25), Note::tone(F3, 15),
    Note::tone(D3, 40), Note::rest(15), Note::tone(A3, 20), Note::tone(D4, 45),
];

/// Track 3: "DMA Echoes" — Original ambient counterpoint in E Minor.
pub static TRACK_3_NOTES: [Note; 26] = [
    Note::tone(E4, 20), Note::tone(G4, 20), Note::tone(B4, 25), Note::tone(E5, 35),
    Note::tone(D5, 20), Note::tone(B4, 20), Note::tone(G4, 25), Note::tone(E4, 30),
    Note::rest(15),

    Note::tone(A4, 20), Note::tone(C5, 20), Note::tone(E5, 30), Note::tone(D5, 20),
    Note::tone(C5, 20), Note::tone(A4, 25), Note::tone(F4, 25), Note::tone(D4, 30),
    Note::rest(15),

    Note::tone(E4, 20), Note::tone(B4, 25), Note::tone(G4, 20), Note::tone(E4, 20),
    Note::tone(B3, 25), Note::tone(E4, 55), Note::rest(20), Note::tone(E5, 45),
];

/// Embedded compact 24 kHz Stereo IMA ADPCM payload for "Mayiiiii" by Maya Rust (150s duration).
pub static MAYIIIII_ADPCM: &[u8] = include_bytes!("mayiiiii_adpcm.bin");

/// Metadata and note stream for a musical track.
#[derive(Debug, Clone, Copy)]
pub struct Track {
    pub title: &'static str,
    pub artist: &'static str,
    pub album: &'static str,
    pub duration_seconds: u16,
    pub notes: &'static [Note],
}

impl Track {
    pub const fn new(
        title: &'static str,
        artist: &'static str,
        album: &'static str,
        duration_seconds: u16,
        notes: &'static [Note],
    ) -> Self {
        Self {
            title,
            artist,
            album,
            duration_seconds,
            notes,
        }
    }

    /// Computes the total duration of the track in timer ticks.
    pub fn total_ticks(&self) -> u32 {
        if self.notes.is_empty() {
            return self.duration_seconds as u32 * 100;
        }
        let mut sum = 0u32;
        for n in self.notes {
            sum += n.duration_ticks as u32;
        }
        sum
    }
}

pub static PLAYLIST: [Track; 4] = [
    Track::new(
        "Mayiiiii",
        "Maya Rust",
        "BubbleOS Sessions",
        150,
        &[], // Streamed from MAYIIIII_ADPCM via AC97 DMA
    ),
    Track::new(
        "Lavender Horizon",
        "Maya Rust",
        "Bare Metal Harmonies",
        6,
        &TRACK_1_NOTES,
    ),
    Track::new(
        "Kernel Pulse",
        "The Interrupts",
        "Ring 0 Symphony",
        5,
        &TRACK_2_NOTES,
    ),
    Track::new(
        "DMA Echoes",
        "82540EM",
        "Gigabit Dreams",
        6,
        &TRACK_3_NOTES,
    ),
];
