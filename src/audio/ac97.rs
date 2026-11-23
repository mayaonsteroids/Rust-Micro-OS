//! Intel ICH AC97 Audio Driver (VirtualBox & QEMU Compatible).
//!
//! Provides genuine bus-master DMA audio streaming with a 32-entry circular
//! descriptor ring, dynamic 48 kHz stereo PCM waveform synthesis, and codec
//! volume management verified for Oracle VirtualBox and QEMU.

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

/// AC97 Buffer Descriptor (8 bytes per entry)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BufferDescriptor {
    pub ptr: u32,       // 32-bit physical address of PCM sample buffer
    pub samples: u16,   // Number of samples (1 frame = 2 samples for 16-bit stereo)
    pub flags: u16,     // Bit 15: IOC (Interrupt on Completion), Bit 14: BUP (Buffer Underrun Policy)
}

/// Global AC97 driver controller
pub struct Ac97Device {
    pub nam_bar: u16,
    pub nabm_bar: u16,
    pub is_active: bool,
    pub bdl_phys: u32,
    pub pcm0_phys: u32,
    pub pcm1_phys: u32,
    pub pcm0_virt: *mut i16,
    pub pcm1_virt: *mut i16,
    pub current_freq: u16,
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

        // 4. Allocate physical memory for BDL and double-buffered PCM audio
        let mut fa_guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        let frame_allocator = fa_guard.as_mut()?;

        let bdl_frame = frame_allocator.allocate_frame()?;
        let pcm0_frame = frame_allocator.allocate_frame()?;
        let pcm1_frame = frame_allocator.allocate_frame()?;
        drop(fa_guard);

        let bdl_phys = bdl_frame.start_address().as_u64() as u32;
        let pcm0_phys = pcm0_frame.start_address().as_u64() as u32;
        let pcm1_phys = pcm1_frame.start_address().as_u64() as u32;

        info!("[AC97-INIT] Allocated DMA Frames: BDL=0x{:08x}, PCM Buf0=0x{:08x}, PCM Buf1=0x{:08x}",
            bdl_phys, pcm0_phys, pcm1_phys);

        let phys_offset = crate::memory::address_space::PHYSICAL_MEMORY_OFFSET
            .load(core::sync::atomic::Ordering::SeqCst);
        let bdl_virt = (phys_offset + bdl_phys as u64) as *mut BufferDescriptor;
        let pcm0_virt = (phys_offset + pcm0_phys as u64) as *mut i16;
        let pcm1_virt = (phys_offset + pcm1_phys as u64) as *mut i16;

        // 5. Populate complete 32-entry circular BDL ring alternating between Buf0 and Buf1
        // 2048 samples per buffer = 1024 stereo frames (4096 bytes) = 21.3 ms at 48 kHz
        unsafe {
            for i in 0..32 {
                let ptr = if (i % 2) == 0 { pcm0_phys } else { pcm1_phys };
                let desc = BufferDescriptor {
                    ptr,
                    samples: 2048,
                    flags: 0,
                };
                *bdl_virt.add(i) = desc;
            }

            // Zero out PCM buffers initially (silence)
            for i in 0..2048 {
                *pcm0_virt.add(i) = 0;
                *pcm1_virt.add(i) = 0;
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
            pcm0_phys,
            pcm1_phys,
            pcm0_virt,
            pcm1_virt,
            current_freq: 0,
        })
    }

    /// Synthesizes 48 kHz 16-bit stereo PCM audio samples into the physical DMA buffers.
    pub fn synthesize_tone(&mut self, freq_hz: u16, volume: u8) {
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
            for frame in 0..1024 {
                let sample: i16 = if freq_hz == 0 {
                    0
                } else if (frame % (half_period * 2)) < half_period {
                    amp
                } else {
                    -amp
                };

                // Left channel
                *self.pcm0_virt.add(frame as usize * 2) = sample;
                *self.pcm1_virt.add(frame as usize * 2) = sample;
                // Right channel
                *self.pcm0_virt.add(frame as usize * 2 + 1) = sample;
                *self.pcm1_virt.add(frame as usize * 2 + 1) = sample;
            }
        }
        self.current_freq = freq_hz;
    }

    /// Starts PCM DMA playback on the AC97 controller.
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
        info!("[AC97] Streaming Started: CIV={}, LVI=31, SR=0x{:04x}, Tone={}Hz", civ, sr, freq_hz);
    }

    /// Pauses PCM DMA playback and clears audio buffers.
    pub fn pause(&mut self) {
        unsafe {
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);
            cr_port.write(0);

            // Zero out audio buffers to eliminate clicks
            for i in 0..2048 {
                *self.pcm0_virt.add(i) = 0;
                *self.pcm1_virt.add(i) = 0;
            }
        }
        self.is_active = false;
        let civ = unsafe { Port::<u8>::new(self.nabm_bar + NABM_PO_CIV).read() };
        info!("[AC97] Streaming Paused: CIV={}", civ);
    }

    /// Advances continuous DMA streaming and ensures LVI stays ahead of CIV without stalling.
    pub fn tick(&mut self, freq_hz: u16, volume: u8) {
        if !self.is_active {
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

        // Also scale active PCM waveform
        self.synthesize_tone(self.current_freq, clamped);
    }
}
