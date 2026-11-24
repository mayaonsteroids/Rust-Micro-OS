//! Graphical Display Subsystem for Rust Micro-OS.
//!
//! Provides genuine 32-bit pixel-rendered graphics via PCI Bochs/VirtualBox
//! Graphics Adapter (BGA) with truecolor primitives, hardware-mapped VRAM,
//! 8x16 bitmap fonts, and handcrafted application icons.

pub mod color;
pub mod cursor;
pub mod draw;
pub mod font;
pub mod framebuffer;
pub mod image;

use spin::Mutex;
pub use color::{theme, Color};
pub use framebuffer::Framebuffer;

/// Global thread-safe Framebuffer singleton.
pub static FRAMEBUFFER: Mutex<Option<Framebuffer>> = Mutex::new(None);

/// Initializes the hardware graphics subsystem with default 1024×768 resolution.
pub fn init() -> bool {
    let mut fb_guard = FRAMEBUFFER.lock();
    if fb_guard.is_some() {
        return true;
    }

    if let Some(fb) = Framebuffer::init(framebuffer::DEFAULT_WIDTH, framebuffer::DEFAULT_HEIGHT) {
        *fb_guard = Some(fb);
        true
    } else {
        false
    }
}

/// Returns true if a linear pixel framebuffer is active.
pub fn is_active() -> bool {
    FRAMEBUFFER.lock().as_ref().map(|fb| fb.is_active).unwrap_or(false)
}

/// Executes a closure with mutable access to the active Framebuffer.
pub fn with_framebuffer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Framebuffer) -> R,
{
    let mut guard = FRAMEBUFFER.lock();
    guard.as_mut().map(f)
}
