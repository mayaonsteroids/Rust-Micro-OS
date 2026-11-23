//! PC Speaker Audio Driver via PIT Channel 2 and I/O Port 0x61.
//!
//! Provides programmable square-wave frequency synthesis, note generation,
//! and hardware speaker gating compatible with all x86 PC hypervisors.

use spin::Mutex;
use x86_64::instructions::port::Port;

const PIT_BASE_FREQUENCY: u32 = 1193182;
const PORT_PIT_CHANNEL2: u16 = 0x42;
const PORT_PIT_MODE: u16 = 0x43;
const PORT_SPEAKER_CONTROL: u16 = 0x61;

/// Global synchronized state for the PC Speaker hardware.
pub static SPEAKER: Mutex<PcSpeaker> = Mutex::new(PcSpeaker::new());

pub struct PcSpeaker {
    is_active: bool,
    current_freq: u16,
}

impl PcSpeaker {
    pub const fn new() -> Self {
        Self {
            is_active: false,
            current_freq: 0,
        }
    }

    /// Plays a tone of specified frequency in Hertz (Hz).
    /// If `freq_hz == 0`, silences the speaker.
    pub fn play_tone(&mut self, freq_hz: u16) {
        if freq_hz == 0 {
            self.silence();
            return;
        }

        let divisor = (PIT_BASE_FREQUENCY / freq_hz as u32) as u16;

        unsafe {
            let mut mode_port = Port::<u8>::new(PORT_PIT_MODE);
            let mut data_port = Port::<u8>::new(PORT_PIT_CHANNEL2);
            let mut control_port = Port::<u8>::new(PORT_SPEAKER_CONTROL);

            // Command 0xB6: Channel 2, LSB then MSB, Mode 3 (Square Wave Generator), 16-bit binary
            mode_port.write(0xB6);

            // Write divisor low byte then high byte
            data_port.write((divisor & 0xFF) as u8);
            data_port.write(((divisor >> 8) & 0xFF) as u8);

            // Enable speaker: set bit 0 (timer 2 gate) and bit 1 (speaker data enable)
            let control = control_port.read();
            if (control & 0x03) != 0x03 {
                control_port.write(control | 0x03);
            }
        }

        self.is_active = true;
        self.current_freq = freq_hz;
    }

    /// Silences the PC speaker by clearing bits 0 and 1 of port 0x61.
    pub fn silence(&mut self) {
        unsafe {
            let mut control_port = Port::<u8>::new(PORT_SPEAKER_CONTROL);
            let control = control_port.read();
            control_port.write(control & 0xFC);
        }
        self.is_active = false;
        self.current_freq = 0;
    }

    /// Returns true if the speaker is actively emitting a tone.
    pub fn is_playing(&self) -> bool {
        self.is_active
    }

    /// Returns the currently playing frequency in Hz.
    pub fn current_frequency(&self) -> u16 {
        self.current_freq
    }
}

/// Plays a tone on the global PC Speaker.
pub fn play_tone(freq_hz: u16) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SPEAKER.lock().play_tone(freq_hz);
    });
}

/// Silences the global PC Speaker.
pub fn silence() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SPEAKER.lock().silence();
    });
}
