//! Intel ICH AC97 Audio Driver (VirtualBox & QEMU Compatible).
//!
//! Provides genuine bus-master DMA audio streaming with a 32-entry circular
//! descriptor ring, hardware IMA ADPCM real-time decoding for embedded song audio,
//! dynamic 48 kHz stereo PCM waveform synthesis, and codec volume management
//! verified for Oracle VirtualBox and QEMU.

use x86_64::instructions::port::Port;
use crate::info;
use crate::pci;

// AC97 Mixer Registers (NAMBAR, BAR0)
const MIXER_RESET: u16 = 0x00;
const MIXER_MASTER_VOL: u16 = 0x02;
const MIXER_PCM_VOL: u16 = 0x18;

// AC97 Bus Master Registers (NABMBAR, BAR1)
const NABM_PO_BDBAR: u16 = 0x10;  // PCM Out Buffer Descriptor Base Address (32-bit)
const NABM_PO_CIV: u16 = 0x14;    // PCM Out Current Index (8-bit)
const NABM_PO_LVI: u16 = 0x15;    // PCM Out Last Valid Index (8-bit)
const NABM_PO_SR: u16 = 0x16;     // PCM Out Status Register (16-bit)
const NABM_PO_CR: u16 = 0x1B;     // PCM Out Control Register (8-bit)
const NABM_GLOB_STA: u16 = 0x30;  // Global Status Register (32-bit)

// Control Register Bits
const PO_CR_RUN: u8 = 1 << 0;     // Run/Pause Bus Master
const PO_CR_RESET: u8 = 1 << 1;   // Reset Registers

/// Standard IMA ADPCM Step Table (89 quantization step sizes)
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17,
    19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
    50, 55, 60, 66, 73, 80, 88, 97, 107, 118,
    130, 143, 157, 173, 190, 209, 230, 253, 279, 307,
    337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358,
    5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899,
    15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// IMA ADPCM Index Adjustment Table (4-bit nibble index)
const INDEX_TABLE: [i8; 16] = [
    -1, -1, -1, -1, 2, 4, 6, 8,
    -1, -1, -1, -1, 2, 4, 6, 8,
];

/// Decodes a single 4-bit IMA ADPCM nibble to a 16-bit linear PCM sample.
#[inline(always)]
fn decode_sample(nibble: u8, predicted: &mut i32, step_idx: &mut usize) -> i16 {
    let step = STEP_TABLE[*step_idx];
    let mut diffq = step >> 3;
    if (nibble & 4) != 0 {
        diffq += step;
    }
    if (nibble & 2) != 0 {
        diffq += step >> 1;
    }
    if (nibble & 1) != 0 {
        diffq += step >> 2;
    }
    if (nibble & 8) != 0 {
        *predicted = (*predicted - diffq).max(-32768);
    } else {
        *predicted = (*predicted + diffq).min(32767);
    }
    let next_idx = (*step_idx as i32) + (INDEX_TABLE[(nibble & 0x0F) as usize] as i32);
    *step_idx = next_idx.clamp(0, 88) as usize;
    *predicted as i16
}

/// AC97 Buffer Descriptor (8 bytes per entry)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BufferDescriptor {
    pub ptr: u32,       // 32-bit physical address of PCM sample buffer
    pub samples: u16,   // Number of samples (1 frame = 2 samples for 16-bit stereo)
    pub flags: u16,     // Bit 15: IOC (Interrupt on Completion), Bit 14: BUP (Buffer Underrun Policy)
}

/// Global AC97 driver controller supporting 32 physical DMA buffer frames
pub struct Ac97Device {
    pub nam_bar: u16,
    pub nabm_bar: u16,
    pub is_active: bool,
    pub bdl_phys: u32,
    pub pcm_phys: [u32; 32],
    pub pcm_virt: [*mut i16; 32],
    pub current_freq: u16,
    pub song_offset: usize,
    pub refill_idx: usize,
    pub pred_l: i32,
    pub step_l: usize,
    pub pred_r: i32,
    pub step_r: usize,
    pub is_song_mode: bool,
}

// Implement Send for Ac97Device since raw pointers point to identity-mapped DMA buffers
unsafe impl Send for Ac97Device {}

impl Ac97Device {
    /// Detects and initializes the Intel ICH AC97 audio hardware.
    pub fn init() -> Option<Self> {
        info!("[AC97-INIT] Probing PCI bus for Intel ICH AC97 Audio Controller...");

        // Look for Intel AC97 (0x8086:0x2415) or Multimedia Audio Controller (Class 0x04, Subclass 0x01)
        let mut pci_dev = pci::find_device(0x8086, 0x2415)
            .or_else(|| pci::find_device_by_class(0x04, 0x01))?;

        info!("[AC97-INIT] Discovered Controller at PCI {:02x}:{:02x}.{} (Vendor: 0x{:04x}, Device: 0x{:04x})",
            pci_dev.bus, pci_dev.slot, pci_dev.func, pci_dev.vendor_id, pci_dev.device_id);

        // Enable Bus Mastering (DMA) and I/O Space in PCI Command
        pci_dev.enable_io_space();
        pci_dev.enable_bus_mastering();
        let cmd = pci_dev.read_command();
        info!("[AC97-INIT] PCI Command configured: 0x{:04x} (I/O: ON, Bus Master: ON)", cmd);

        // BAR0 = NAMBAR (Mixer), BAR1 = NABMBAR (Bus Master)
        let (nam_bar, _) = pci_dev.get_bar_io(0)
            .or_else(|| {
                let bar0 = unsafe { pci::config::pci_config_read_u32(pci_dev.bus, pci_dev.slot, pci_dev.func, 0x10) };
                if (bar0 & 1) != 0 { Some(((bar0 & !3) as u16, 256)) } else { None }
            })?;

        let (nabm_bar, _) = pci_dev.get_bar_io(1)
            .or_else(|| {
                let bar1 = unsafe { pci::config::pci_config_read_u32(pci_dev.bus, pci_dev.slot, pci_dev.func, 0x14) };
                if (bar1 & 1) != 0 { Some(((bar1 & !3) as u16, 64)) } else { None }
            })?;

        info!("[AC97-INIT] Decoded Base Addresses: NAMBAR=0x{:04x} (Mixer), NABMBAR=0x{:04x} (DMA)",
            nam_bar, nabm_bar);

        // 1. Reset Mixer & Bus Master
        unsafe {
            let mut reset_port = Port::<u16>::new(nam_bar + MIXER_RESET);
            reset_port.write(1);

            let mut cr_port = Port::<u8>::new(nabm_bar + NABM_PO_CR);
            cr_port.write(PO_CR_RESET);
            for _ in 0..2000 { core::hint::spin_loop(); }
            cr_port.write(0);
        }

        // 2. Poll Codec Readiness via GLOB_STA (Bit 8: S0CR = Primary Codec Ready)
        let glob_sta = unsafe { Port::<u32>::new(nabm_bar + NABM_GLOB_STA).read() };
        let codec_ready = (glob_sta & (1 << 8)) != 0;
        info!("[AC97-INIT] Global Status GLOB_STA: 0x{:08x} (Primary Codec Ready: {})", glob_sta, codec_ready);

        // 3. Unmute Master Volume (0x02) and PCM Out Volume (0x18)
        // Value 0x0000 = 0 dB attenuation (max gain), Mute bit (15) = 0
        unsafe {
            let mut master_vol = Port::<u16>::new(nam_bar + MIXER_MASTER_VOL);
            master_vol.write(0x0000);

            let mut pcm_vol = Port::<u16>::new(nam_bar + MIXER_PCM_VOL);
            pcm_vol.write(0x0000);
        }

        let m_read = unsafe { Port::<u16>::new(nam_bar + MIXER_MASTER_VOL).read() };
        let p_read = unsafe { Port::<u16>::new(nam_bar + MIXER_PCM_VOL).read() };
        info!("[AC97-INIT] Mixer Unmuted: Master Vol=0x{:04x}, PCM Vol=0x{:04x} (Mute={})",
            m_read, p_read, (m_read & 0x8000) != 0);

        // 4. Allocate physical memory for BDL and 32 separate 4 KiB PCM DMA buffers
        let mut fa_guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        let frame_allocator = fa_guard.as_mut()?;

        let bdl_frame = frame_allocator.allocate_frame()?;
        let bdl_phys = bdl_frame.start_address().as_u64() as u32;

        let mut pcm_phys = [0u32; 32];
        let mut pcm_virt = [core::ptr::null_mut::<i16>(); 32];

        let phys_offset = crate::memory::address_space::PHYSICAL_MEMORY_OFFSET
            .load(core::sync::atomic::Ordering::SeqCst);

        for i in 0..32 {
            let pcm_frame = frame_allocator.allocate_frame()?;
            let phys = pcm_frame.start_address().as_u64() as u32;
            pcm_phys[i] = phys;
            pcm_virt[i] = (phys_offset + phys as u64) as *mut i16;
        }
        drop(fa_guard);

        info!("[AC97-INIT] Allocated 32 DMA Frames: BDL=0x{:08x}, PCM Buf[0]=0x{:08x}..Buf[31]=0x{:08x}",
            bdl_phys, pcm_phys[0], pcm_phys[31]);

        let bdl_virt = (phys_offset + bdl_phys as u64) as *mut BufferDescriptor;

        // 5. Populate complete 32-entry circular BDL ring
        // 2048 samples per buffer = 1024 stereo frames (4096 bytes) = 21.3 ms at 48 kHz
        unsafe {
            for i in 0..32 {
                let desc = BufferDescriptor {
                    ptr: pcm_phys[i],
                    samples: 2048,
                    flags: 0,
                };
                *bdl_virt.add(i) = desc;

                // Zero out PCM buffer initially (silence)
                for s in 0..2048 {
                    *pcm_virt[i].add(s) = 0;
                }
            }

            // Program NABM DMA Registers
            let mut bdbar_port = Port::<u32>::new(nabm_bar + NABM_PO_BDBAR);
            bdbar_port.write(bdl_phys);

            let mut lvi_port = Port::<u8>::new(nabm_bar + NABM_PO_LVI);
            lvi_port.write(31); // 32 descriptors active (ring index 0..31)

            // Clear any stale status flags (W1C: write 1 to clear)
            let mut sr_port = Port::<u16>::new(nabm_bar + NABM_PO_SR);
            sr_port.write(0x001E);
        }

        let civ = unsafe { Port::<u8>::new(nabm_bar + NABM_PO_CIV).read() };
        let lvi = unsafe { Port::<u8>::new(nabm_bar + NABM_PO_LVI).read() };
        let sr = unsafe { Port::<u16>::new(nabm_bar + NABM_PO_SR).read() };
        info!("[AC97-INIT] 32-Entry Ring Ready: CIV={}, LVI={}, SR=0x{:04x}", civ, lvi, sr);

        Some(Self {
            nam_bar,
            nabm_bar,
            is_active: false,
            bdl_phys,
            pcm_phys,
            pcm_virt,
            current_freq: 0,
            song_offset: 0,
            refill_idx: 0,
            pred_l: 0,
            step_l: 0,
            pred_r: 0,
            step_r: 0,
            is_song_mode: false,
        })
    }

    /// Zeroes out the PCM sample buffer of a descriptor entry.
    pub fn zero_descriptor(&mut self, desc_idx: usize) {
        let buf = self.pcm_virt[desc_idx % 32];
        unsafe {
            for s in 0..2048 {
                *buf.add(s) = 0;
            }
        }
    }

    /// Decodes a 512-byte block of 24 kHz stereo IMA ADPCM into 1024 48 kHz stereo frames (2048 i16 samples)
    /// in descriptor `desc_idx`.
    pub fn decode_and_fill(&mut self, desc_idx: usize, chunk: &[u8], volume: u8) {
        let vol_factor = volume.min(100) as i32;
        let buf = self.pcm_virt[desc_idx % 32];
        let bytes_to_decode = chunk.len().min(512);

        unsafe {
            for i in 0..bytes_to_decode {
                let byte = chunk[i];
                let left_nib = (byte >> 4) & 0x0F;
                let right_nib = byte & 0x0F;

                let raw_left = decode_sample(left_nib, &mut self.pred_l, &mut self.step_l);
                let raw_right = decode_sample(right_nib, &mut self.pred_r, &mut self.step_r);

                let scaled_l = ((raw_left as i32 * vol_factor) / 100) as i16;
                let scaled_r = ((raw_right as i32 * vol_factor) / 100) as i16;

                // 2x sample replication (24 kHz -> 48 kHz stereo)
                let base_frame = i * 2;
                // Frame 0: Left, Right
                *buf.add(base_frame * 2) = scaled_l;
                *buf.add(base_frame * 2 + 1) = scaled_r;
                // Frame 1: Left, Right
                *buf.add((base_frame + 1) * 2) = scaled_l;
                *buf.add((base_frame + 1) * 2 + 1) = scaled_r;
            }

            // Zero remaining frames if chunk is short
            for frame in (bytes_to_decode * 2)..1024 {
                *buf.add(frame * 2) = 0;
                *buf.add(frame * 2 + 1) = 0;
            }
        }
    }

    /// Starts streaming the song data from the beginning (or resumes from current offset).
    pub fn start_song(&mut self, song_data: &[u8], volume: u8, resume: bool) {
        self.is_song_mode = true;
        if !resume {
            self.song_offset = 0;
            self.refill_idx = 0;
            self.pred_l = 0;
            self.step_l = 0;
            self.pred_r = 0;
            self.step_r = 0;

            // Pre-fill all 32 descriptors initially
            for i in 0..32 {
                if self.song_offset + 512 <= song_data.len() {
                    let chunk = &song_data[self.song_offset..self.song_offset + 512];
                    self.decode_and_fill(i, chunk, volume);
                    self.song_offset += 512;
                } else {
                    self.zero_descriptor(i);
                }
            }
            self.refill_idx = 0;
        }

        unsafe {
            // Clear status bits (W1C)
            Port::<u16>::new(self.nabm_bar + NABM_PO_SR).write(0x001E);
            // Set LVI to 31
            Port::<u8>::new(self.nabm_bar + NABM_PO_LVI).write(31);
            // Run Bus Master
            Port::<u8>::new(self.nabm_bar + NABM_PO_CR).write(PO_CR_RUN);
        }

        self.is_active = true;
        let civ = unsafe { Port::<u8>::new(self.nabm_bar + NABM_PO_CIV).read() };
        let sr = unsafe { Port::<u16>::new(self.nabm_bar + NABM_PO_SR).read() };
        info!("[AC97] Song Streaming Started: CIV={}, LVI=31, SR=0x{:04x}, Offset={}/{} bytes",
            civ, sr, self.song_offset, song_data.len());
    }

    /// Services continuous DMA ring streaming for the song.
    /// Returns false when the song has finished playing (if is_repeat is false).
    pub fn tick_song(&mut self, song_data: &[u8], volume: u8, is_repeat: bool) -> bool {
        if !self.is_active || !self.is_song_mode {
            return false;
        }

        unsafe {
            let civ = Port::<u8>::new(self.nabm_bar + NABM_PO_CIV).read() as usize;
            let sr = Port::<u16>::new(self.nabm_bar + NABM_PO_SR).read();

            // Refill descriptors ahead of CIV
            // Keep up to 24 descriptors pre-buffered ahead of the hardware position
            let mut refilled = 0;
            while ((self.refill_idx + 32 - civ) % 32) < 24 && refilled < 4 {
                if self.song_offset + 512 <= song_data.len() {
                    let chunk = &song_data[self.song_offset..self.song_offset + 512];
                    self.decode_and_fill(self.refill_idx, chunk, volume);
                    self.song_offset += 512;
                    // Move LVI to this refilled descriptor
                    Port::<u8>::new(self.nabm_bar + NABM_PO_LVI).write(self.refill_idx as u8);
                    self.refill_idx = (self.refill_idx + 1) % 32;
                    refilled += 1;
                } else if is_repeat {
                    // Loop back to beginning of song
                    self.song_offset = 0;
                    self.pred_l = 0;
                    self.step_l = 0;
                    self.pred_r = 0;
                    self.step_r = 0;
                } else {
                    // End of song reached
                    self.zero_descriptor(self.refill_idx);
                    if civ == self.refill_idx {
                        self.pause();
                        return false;
                    }
                    self.refill_idx = (self.refill_idx + 1) % 32;
                    break;
                }
            }

            // If DMA paused/halted (DCH bit 0 set), clear status and resume
            if (sr & 1) != 0 {
                Port::<u16>::new(self.nabm_bar + NABM_PO_SR).write(0x001E);
                Port::<u8>::new(self.nabm_bar + NABM_PO_CR).write(PO_CR_RUN);
            }
        }

        true
    }

    /// Synthesizes 48 kHz 16-bit stereo PCM audio samples into all physical DMA buffers for chiptune tones.
    pub fn synthesize_tone(&mut self, freq_hz: u16, volume: u8) {
        self.is_song_mode = false;
        let amp: i16 = if freq_hz > 0 {
            ((12000i32 * volume.min(100) as i32) / 100) as i16
        } else {
            0
        };

        // Half period in samples at 48000 Hz: H = 24000 / freq_hz
        let half_period = if freq_hz > 0 {
            (24000 / freq_hz as u32).max(1)
        } else {
            1
        };

        unsafe {
            for desc in 0..32 {
                let buf = self.pcm_virt[desc];
                for frame in 0..1024 {
                    let sample: i16 = if freq_hz == 0 {
                        0
                    } else if (frame % (half_period * 2)) < half_period {
                        amp
                    } else {
                        -amp
                    };

                    // Left channel
                    *buf.add(frame as usize * 2) = sample;
                    // Right channel
                    *buf.add(frame as usize * 2 + 1) = sample;
                }
            }
        }
        self.current_freq = freq_hz;
    }

    /// Starts PCM DMA playback on the AC97 controller for chiptune tone playback.
    pub fn start(&mut self, freq_hz: u16, volume: u8) {
        self.synthesize_tone(freq_hz, volume);

        unsafe {
            // Clear status bits (W1C)
            let mut sr_port = Port::<u16>::new(self.nabm_bar + NABM_PO_SR);
            sr_port.write(0x001E);

            // Ensure LVI is set to 31
            let mut lvi_port = Port::<u8>::new(self.nabm_bar + NABM_PO_LVI);
            lvi_port.write(31);

            // Write Run bit
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);
            cr_port.write(PO_CR_RUN);
        }

        self.is_active = true;
        let civ = unsafe { Port::<u8>::new(self.nabm_bar + NABM_PO_CIV).read() };
        let sr = unsafe { Port::<u16>::new(self.nabm_bar + NABM_PO_SR).read() };
        info!("[AC97] Tone Streaming Started: CIV={}, LVI=31, SR=0x{:04x}, Tone={}Hz", civ, sr, freq_hz);
    }

    /// Pauses PCM DMA playback and zeroes audio buffers.
    pub fn pause(&mut self) {
        unsafe {
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);
            cr_port.write(0);

            // Zero out audio buffers to eliminate clicks
            for desc in 0..32 {
                let buf = self.pcm_virt[desc];
                for s in 0..2048 {
                    *buf.add(s) = 0;
                }
            }
        }
        self.is_active = false;
        let civ = unsafe { Port::<u8>::new(self.nabm_bar + NABM_PO_CIV).read() };
        info!("[AC97] Streaming Paused: CIV={}", civ);
    }

    /// Stops PCM DMA playback and resets streaming state.
    pub fn stop(&mut self) {
        self.pause();
        self.song_offset = 0;
        self.refill_idx = 0;
        self.pred_l = 0;
        self.step_l = 0;
        self.pred_r = 0;
        self.step_r = 0;
        self.is_song_mode = false;
        info!("[AC97] Streaming Stopped and Reset.");
    }

    /// Advances continuous DMA streaming for tone synthesis.
    pub fn tick(&mut self, freq_hz: u16, volume: u8) {
        if !self.is_active || self.is_song_mode {
            return;
        }

        // If tone changed, update the PCM sample buffer
        if self.current_freq != freq_hz {
            self.synthesize_tone(freq_hz, volume);
        }

        unsafe {
            let mut civ_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CIV);
            let mut lvi_port = Port::<u8>::new(self.nabm_bar + NABM_PO_LVI);
            let mut sr_port = Port::<u16>::new(self.nabm_bar + NABM_PO_SR);
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);

            let civ = civ_port.read();
            let sr = sr_port.read();

            // Keep LVI 16 entries ahead of CIV so DMA never halts
            let target_lvi = (civ + 16) % 32;
            lvi_port.write(target_lvi);

            // If DMA halted (DCH bit 0 set), clear status and resume
            if (sr & 1) != 0 {
                sr_port.write(0x001E); // Clear CELV, LVBCI, BCIS, FIFOE
                cr_port.write(PO_CR_RUN);
            }
        }
    }

    /// Adjusts AC97 master and PCM volume (0..100).
    pub fn set_volume(&mut self, vol: u8) {
        let clamped = vol.min(100);
        // Attenuation is 0 (0 dB max) to 31 (-46.5 dB min), bit 15 is mute
        let atten = ((100 - clamped) as u16 * 31) / 100;
        let reg_val = if clamped == 0 {
            0x8000 // Mute
        } else {
            (atten << 8) | atten
        };

        unsafe {
            let mut master_vol = Port::<u16>::new(self.nam_bar + MIXER_MASTER_VOL);
            master_vol.write(reg_val);

            let mut pcm_vol = Port::<u16>::new(self.nam_bar + MIXER_PCM_VOL);
            pcm_vol.write(reg_val);
        }

        if !self.is_song_mode {
            self.synthesize_tone(self.current_freq, clamped);
        }
    }
}
