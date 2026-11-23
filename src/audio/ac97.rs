//! Intel ICH AC97 Audio Driver (VirtualBox & QEMU Compatible).
//!
//! Controls the AC'97 mixer and Native Audio Bus Master (NABM) DMA engine,
//! unmuting master volume, setting up PCM descriptor rings, and streaming audio.

use x86_64::instructions::port::Port;
use crate::info;
use crate::pci;

// AC97 Mixer Registers (NAMBAR, BAR0)
const MIXER_RESET: u16 = 0x00;
const MIXER_MASTER_VOL: u16 = 0x02;
const MIXER_PCM_VOL: u16 = 0x18;

// AC97 Bus Master Registers (NABMBAR, BAR1)
const NABM_PO_BDBAR: u16 = 0x10; // PCM Out Buffer Descriptor Base Address (32-bit)
#[allow(dead_code)]
const NABM_PO_CIV: u16 = 0x14;   // PCM Out Current Index (8-bit)
const NABM_PO_LVI: u16 = 0x15;   // PCM Out Last Valid Index (8-bit)
#[allow(dead_code)]
const NABM_PO_SR: u16 = 0x16;    // PCM Out Status Register (16-bit)
const NABM_PO_CR: u16 = 0x1B;    // PCM Out Control Register (8-bit)

// Control Register Bits
const PO_CR_RUN: u8 = 1 << 0;    // Run/Pause Bus Master
const PO_CR_RESET: u8 = 1 << 1;  // Reset Registers

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
    pub pcm_phys: u32,
}

impl Ac97Device {
    /// Detects and initializes the Intel ICH AC97 audio hardware.
    pub fn init() -> Option<Self> {
        // Look for Intel AC97 (0x8086:0x2415) or Multimedia Audio Controller (Class 0x04, Subclass 0x01)
        let mut pci_dev = pci::find_device(0x8086, 0x2415)
            .or_else(|| pci::find_device_by_class(0x04, 0x01))?;

        info!("[AC97] Discovered AC97 Controller at PCI {:02x}:{:02x}.{}",
            pci_dev.bus, pci_dev.slot, pci_dev.func);

        // Enable Bus Mastering (DMA) and I/O Space in PCI Command
        pci_dev.enable_io_space();
        pci_dev.enable_bus_mastering();
        let cmd = pci_dev.read_command();
        info!("[AC97] Configured PCI Command: 0x{:04x} (I/O: ON, Bus Master: ON)", cmd);

        // BAR0 = NAMBAR (Mixer), BAR1 = NABMBAR (Bus Master)
        let (nam_bar, _) = pci_dev.get_bar_io(0)
            .or_else(|| {
                // If not parsed as I/O BAR enum, read port directly from PCI config
                let bar0 = unsafe { pci::config::pci_config_read_u32(pci_dev.bus, pci_dev.slot, pci_dev.func, 0x10) };
                if (bar0 & 1) != 0 { Some(((bar0 & !3) as u16, 256)) } else { None }
            })?;

        let (nabm_bar, _) = pci_dev.get_bar_io(1)
            .or_else(|| {
                let bar1 = unsafe { pci::config::pci_config_read_u32(pci_dev.bus, pci_dev.slot, pci_dev.func, 0x14) };
                if (bar1 & 1) != 0 { Some(((bar1 & !3) as u16, 64)) } else { None }
            })?;

        info!("[AC97] NAMBAR (Mixer) at I/O 0x{:04x}, NABMBAR (Bus Master) at I/O 0x{:04x}",
            nam_bar, nabm_bar);

        // 1. Reset Mixer & Bus Master
        unsafe {
            let mut reset_port = Port::<u16>::new(nam_bar + MIXER_RESET);
            reset_port.write(1);

            let mut cr_port = Port::<u8>::new(nabm_bar + NABM_PO_CR);
            cr_port.write(PO_CR_RESET);
            // Brief pause for reset
            for _ in 0..1000 { core::hint::spin_loop(); }
            cr_port.write(0);

            // 2. Unmute Master Volume (0x02) and PCM Out Volume (0x18)
            // Value 0x0000 = 0 dB attenuation (max volume), Mute bit (15) = 0
            let mut master_vol = Port::<u16>::new(nam_bar + MIXER_MASTER_VOL);
            master_vol.write(0x0000);

            let mut pcm_vol = Port::<u16>::new(nam_bar + MIXER_PCM_VOL);
            pcm_vol.write(0x0000);
        }

        info!("[AC97] Mixer unmuted: Master Volume and PCM Output set to 0 dB max.");

        // 3. Allocate physical memory for BDL (Buffer Descriptor List) and PCM samples
        let mut fa_guard = crate::memory::address_space::FRAME_ALLOCATOR.lock();
        let frame_allocator = fa_guard.as_mut()?;

        let pcm_frame = frame_allocator.allocate_frame()?;
        let bdl_frame = frame_allocator.allocate_frame()?;
        drop(fa_guard);

        let pcm_phys = pcm_frame.start_address().as_u64() as u32;
        let bdl_phys = bdl_frame.start_address().as_u64() as u32;

        info!("[AC97] Allocated DMA frames: PCM Buffer at 0x{:08x}, BDL at 0x{:08x}",
            pcm_phys, bdl_phys);

        // Synthesize test 16-bit 44.1 kHz PCM audio waveform in the PCM buffer
        let phys_offset = crate::memory::address_space::PHYSICAL_MEMORY_OFFSET
            .load(core::sync::atomic::Ordering::SeqCst);
        let pcm_virt = (phys_offset + pcm_phys as u64) as *mut i16;
        let bdl_virt = (phys_offset + bdl_phys as u64) as *mut BufferDescriptor;

        // Generate a 440 Hz square wave in the 4096-byte (2048-sample / 1024 stereo frame) buffer
        // 44100 / 440 = ~100 samples per cycle
        unsafe {
            for i in 0..1024 {
                let sample: i16 = if (i % 100) < 50 { 6000 } else { -6000 };
                *pcm_virt.add(i * 2) = sample;     // Left channel
                *pcm_virt.add(i * 2 + 1) = sample; // Right channel
            }

            // Setup BDL: 2 chained descriptors of 2048 samples each (total 4096 bytes)
            let desc0 = BufferDescriptor {
                ptr: pcm_phys,
                samples: 2048,
                flags: 0,
            };
            let desc1 = BufferDescriptor {
                ptr: pcm_phys,
                samples: 2048,
                flags: 1 << 14, // BUP (loop/continue)
            };
            *bdl_virt = desc0;
            *bdl_virt.add(1) = desc1;

            // Program NABM registers
            let mut bdbar_port = Port::<u32>::new(nabm_bar + NABM_PO_BDBAR);
            bdbar_port.write(bdl_phys);

            let mut lvi_port = Port::<u8>::new(nabm_bar + NABM_PO_LVI);
            lvi_port.write(1); // 2 descriptors (index 0 and 1)
        }

        info!("[AC97] Hardware DMA engine initialized and ready for streaming.");

        Some(Self {
            nam_bar,
            nabm_bar,
            is_active: false,
            bdl_phys,
            pcm_phys,
        })
    }

    /// Starts PCM DMA playback on the AC97 controller.
    pub fn start(&mut self) {
        unsafe {
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);
            cr_port.write(PO_CR_RUN);
        }
        self.is_active = true;
    }

    /// Pauses PCM DMA playback on the AC97 controller.
    pub fn pause(&mut self) {
        unsafe {
            let mut cr_port = Port::<u8>::new(self.nabm_bar + NABM_PO_CR);
            cr_port.write(0);
        }
        self.is_active = false;
    }

    /// Adjusts AC97 master volume (0..100).
    pub fn set_volume(&mut self, vol: u8) {
        let clamped = vol.min(100);
        // Attenuation is 0 (max) to 31 (silent), bit 15 is mute
        let atten = ((100 - clamped) as u16 * 31) / 100;
        let reg_val = (atten << 8) | atten;

        unsafe {
            let mut master_vol = Port::<u16>::new(self.nam_bar + MIXER_MASTER_VOL);
            master_vol.write(reg_val);

            let mut pcm_vol = Port::<u16>::new(self.nam_bar + MIXER_PCM_VOL);
            pcm_vol.write(reg_val);
        }
    }
}
