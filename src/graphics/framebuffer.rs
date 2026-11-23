//! Hardware Framebuffer Driver via Bochs/VirtualBox BGA & PCI MMIO.
//!
//! Provides hardware resolution programming, linear video memory mapping,
//! bounds-checked pixel writes, rectangular filling, clipping rectangles,
//! and dirty region tracking.

use x86_64::instructions::port::Port;
use x86_64::PhysAddr;
use crate::graphics::color::Color;
use crate::info;
use crate::pci;

// Bochs / QEMU / VirtualBox VBE Display Interface I/O Ports
const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

// BGA Register Indices
const VBE_DISPI_INDEX_ID: u16 = 0x0;
const VBE_DISPI_INDEX_XRES: u16 = 0x1;
const VBE_DISPI_INDEX_YRES: u16 = 0x2;
const VBE_DISPI_INDEX_BPP: u16 = 0x3;
const VBE_DISPI_INDEX_ENABLE: u16 = 0x4;
#[allow(dead_code)]
const VBE_DISPI_INDEX_BANK: u16 = 0x5;
#[allow(dead_code)]
const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 0x6;
#[allow(dead_code)]
const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x7;
#[allow(dead_code)]
const VBE_DISPI_INDEX_X_OFFSET: u16 = 0x8;
#[allow(dead_code)]
const VBE_DISPI_INDEX_Y_OFFSET: u16 = 0x9;

// BGA Enable Flags
#[allow(dead_code)]
const VBE_DISPI_DISABLED: u16 = 0x00;
#[allow(dead_code)]
const VBE_DISPI_ENABLED: u16 = 0x01;
#[allow(dead_code)]
const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
#[allow(dead_code)]
const VBE_DISPI_NOCLEARMEM: u16 = 0x20;

// BGA Identification Constants
const VBE_DISPI_ID0: u16 = 0xB0C0;
const VBE_DISPI_ID5: u16 = 0xB0C5;

// Default Desktop Dimensions
pub const DEFAULT_WIDTH: usize = 1024;
pub const DEFAULT_HEIGHT: usize = 768;
pub const DEFAULT_BPP: usize = 32;

/// Global Framebuffer Controller
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pitch: usize,       // Bytes per scanline
    pub bpp: usize,         // Bits per pixel (32)
    pub phys_addr: u64,
    pub base_ptr: *mut u32,
    pub is_active: bool,
    pub clip_x0: usize,
    pub clip_y0: usize,
    pub clip_x1: usize,
    pub clip_y1: usize,
}

// Implement Send & Sync for Framebuffer since base_ptr points to MMIO memory
unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

impl Framebuffer {
    /// Creates an uninitialized Framebuffer instance.
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            pitch: 0,
            bpp: 0,
            phys_addr: 0,
            base_ptr: core::ptr::null_mut(),
            is_active: false,
            clip_x0: 0,
            clip_y0: 0,
            clip_x1: 0,
            clip_y1: 0,
        }
    }

    /// Reads a 16-bit word from a BGA index register.
    #[inline]
    pub unsafe fn bga_read(index: u16) -> u16 {
        let mut index_port = Port::<u16>::new(VBE_DISPI_IOPORT_INDEX);
        let mut data_port = Port::<u16>::new(VBE_DISPI_IOPORT_DATA);
        index_port.write(index);
        data_port.read()
    }

    /// Writes a 16-bit word to a BGA index register.
    #[inline]
    pub unsafe fn bga_write(index: u16, val: u16) {
        let mut index_port = Port::<u16>::new(VBE_DISPI_IOPORT_INDEX);
        let mut data_port = Port::<u16>::new(VBE_DISPI_IOPORT_DATA);
        index_port.write(index);
        data_port.write(val);
    }

    /// Detects the display controller via BGA registers and PCI configuration space,
    /// sets the target resolution, maps the linear framebuffer, and returns initialized state.
    pub fn init(target_w: usize, target_h: usize) -> Option<Self> {
        info!("========================================================");
        info!("           INITIALIZING GRAPHICS FRAMEBUFFER            ");
        info!("========================================================");

        // 1. Check for Bochs / VirtualBox VBE Display Interface presence
        unsafe {
            let id = Self::bga_read(VBE_DISPI_INDEX_ID);
            info!("[GRAPHICS] Probing BGA Interface (0x01CE): Version ID = 0x{:04x}", id);

            if id < VBE_DISPI_ID0 || id > VBE_DISPI_ID5 {
                // Try writing ID5 and reading back to negotiate
                Self::bga_write(VBE_DISPI_INDEX_ID, VBE_DISPI_ID5);
                let check_id = Self::bga_read(VBE_DISPI_INDEX_ID);
                if check_id < VBE_DISPI_ID0 || check_id > VBE_DISPI_ID5 {
                    info!("[-] BGA Graphics Adapter not detected on ports 0x01CE/0x01CF.");
                    return None;
                }
            }
        }

        // 2. Locate PCI Display Controller (Class 0x03, Subclass 0x00) for BAR0 physical address
        let mut pci_bar0: u64 = 0;

        if let Some(mut dev) = pci::find_device_by_class(0x03, 0x00) {
            info!("[GRAPHICS] Found PCI Display Adapter at {:02x}:{:02x}.{} (Vendor: 0x{:04x}, Device: 0x{:04x})",
                dev.bus, dev.slot, dev.func, dev.vendor_id, dev.device_id);

            // Enable Memory Space & Bus Mastering
            dev.enable_memory_space();
            dev.enable_bus_mastering();

            // Extract BAR0 memory base
            if let Some(pci::Bar::Memory { base_addr, .. }) = dev.bars.first() {
                pci_bar0 = *base_addr;
            } else {
                let raw_bar0 = unsafe { pci::config::pci_config_read_u32(dev.bus, dev.slot, dev.func, 0x10) };
                pci_bar0 = (raw_bar0 & !0xF) as u64;
            }
        }

        // Fallback default for Bochs/VirtualBox standard LFB if PCI BAR probing returns 0
        if pci_bar0 == 0 {
            pci_bar0 = 0xE000_0000;
            info!("[GRAPHICS] Using standard fallback LFB physical address 0x{:08x}", pci_bar0);
        } else {
            info!("[GRAPHICS] Decoded PCI BAR0 Framebuffer Address: 0x{:08x}", pci_bar0);
        }

        // 3. Program BGA Video Mode (1024x768x32)
        unsafe {
            // Disable while configuring
            Self::bga_write(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
            Self::bga_write(VBE_DISPI_INDEX_XRES, target_w as u16);
            Self::bga_write(VBE_DISPI_INDEX_YRES, target_h as u16);
            Self::bga_write(VBE_DISPI_INDEX_BPP, DEFAULT_BPP as u16);
            Self::bga_write(VBE_DISPI_INDEX_VIRT_WIDTH, target_w as u16);
            Self::bga_write(VBE_DISPI_INDEX_VIRT_HEIGHT, target_h as u16);
            Self::bga_write(VBE_DISPI_INDEX_X_OFFSET, 0);
            Self::bga_write(VBE_DISPI_INDEX_Y_OFFSET, 0);

            // Enable with Linear Frame Buffer (LFB)
            Self::bga_write(
                VBE_DISPI_INDEX_ENABLE,
                VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED,
            );
        }

        // Verify configured mode
        let cur_x = unsafe { Self::bga_read(VBE_DISPI_INDEX_XRES) } as usize;
        let cur_y = unsafe { Self::bga_read(VBE_DISPI_INDEX_YRES) } as usize;
        let cur_bpp = unsafe { Self::bga_read(VBE_DISPI_INDEX_BPP) } as usize;
        let cur_en = unsafe { Self::bga_read(VBE_DISPI_INDEX_ENABLE) };

        info!("[GRAPHICS] Mode Activated: {}x{} @ {} bpp (Enable Register: 0x{:04x})",
            cur_x, cur_y, cur_bpp, cur_en);

        // 4. Map physical VRAM into virtual memory
        let fb_size = cur_x * cur_y * (cur_bpp / 8);
        info!("[GRAPHICS] Mapping {} bytes of VRAM into virtual address space...", fb_size);

        let virt_base = crate::memory::paging::map_mmio_range(PhysAddr::new(pci_bar0 as u64), fb_size);
        let base_ptr = virt_base.as_mut_ptr::<u32>();

        info!("[GRAPHICS] Framebuffer mapped at virtual address 0x{:016x}", virt_base.as_u64());

        let mut fb = Self {
            width: cur_x,
            height: cur_y,
            pitch: cur_x * 4,
            bpp: cur_bpp,
            phys_addr: pci_bar0,
            base_ptr,
            is_active: true,
            clip_x0: 0,
            clip_y0: 0,
            clip_x1: cur_x,
            clip_y1: cur_y,
        };

        // Clear screen with theme desktop background (#E9DFFF)
        fb.clear(crate::graphics::color::theme::DESKTOP_BG);

        Some(fb)
    }

    /// Sets the active clipping rectangle for drawing operations.
    pub fn set_clip_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        self.clip_x0 = x0.min(self.width);
        self.clip_y0 = y0.min(self.height);
        self.clip_x1 = x1.min(self.width);
        self.clip_y1 = y1.min(self.height);
    }

    /// Resets the clipping rectangle to the entire screen.
    pub fn reset_clip_rect(&mut self) {
        self.clip_x0 = 0;
        self.clip_y0 = 0;
        self.clip_x1 = self.width;
        self.clip_y1 = self.height;
    }

    /// Sets a single pixel to `color`, respecting clipping boundaries.
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.clip_x0 || x >= self.clip_x1 || y < self.clip_y0 || y >= self.clip_y1 {
            return;
        }

        let offset = y * (self.pitch / 4) + x;
        unsafe {
            if color.a == 255 {
                *self.base_ptr.add(offset) = color.to_u32();
            } else if color.a > 0 {
                let existing_u32 = *self.base_ptr.add(offset);
                let existing_color = Color::rgba(
                    ((existing_u32 >> 16) & 0xFF) as u8,
                    ((existing_u32 >> 8) & 0xFF) as u8,
                    (existing_u32 & 0xFF) as u8,
                    255,
                );
                let blended = color.blend_over(existing_color);
                *self.base_ptr.add(offset) = blended.to_u32();
            }
        }
    }

    /// Reads a single pixel at `(x, y)`.
    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        if x >= self.width || y >= self.height {
            return Color::rgb(0, 0, 0);
        }
        let offset = y * (self.pitch / 4) + x;
        let u = unsafe { *self.base_ptr.add(offset) };
        Color::rgba(
            ((u >> 16) & 0xFF) as u8,
            ((u >> 8) & 0xFF) as u8,
            (u & 0xFF) as u8,
            255,
        )
    }

    /// Clears the entire framebuffer to a solid color.
    pub fn clear(&mut self, color: Color) {
        let val = color.to_u32();
        let total_pixels = self.width * self.height;
        unsafe {
            for i in 0..total_pixels {
                *self.base_ptr.add(i) = val;
            }
        }
    }

    /// Fills a solid rectangle with `color`, clipped to bounds.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        let x_end = (x + w).min(self.clip_x1);
        let y_end = (y + h).min(self.clip_y1);
        let x_start = x.max(self.clip_x0);
        let y_start = y.max(self.clip_y0);

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        let val = color.to_u32();
        let words_per_line = self.pitch / 4;

        if color.a == 255 {
            for row in y_start..y_end {
                let row_offset = row * words_per_line;
                for col in x_start..x_end {
                    unsafe {
                        *self.base_ptr.add(row_offset + col) = val;
                    }
                }
            }
        } else {
            for row in y_start..y_end {
                for col in x_start..x_end {
                    self.set_pixel(col, row, color);
                }
            }
        }
    }
}
