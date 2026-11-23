//! 32-Bit Truecolor types and Light Lavender theme palette.
//!
//! Provides RGBA color representations, color conversions, blending math,
//! and the standardized Micro-OS desktop color scheme.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Creates a new Color with full opacity (alpha = 255).
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a new Color with specified alpha (0 = transparent, 255 = opaque).
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a Color from a 24-bit RGB integer (0xRRGGBB).
    pub const fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as u8;
        let g = ((hex >> 8) & 0xFF) as u8;
        let b = (hex & 0xFF) as u8;
        Self { r, g, b, a: 255 }
    }

    /// Returns the 32-bit pixel value for linear framebuffers (standard BGRA little-endian format).
    #[inline(always)]
    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Alpha-blends `self` (foreground) over `bg` (background).
    #[inline]
    pub fn blend_over(self, bg: Color) -> Color {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return bg;
        }

        let alpha = self.a as u32;
        let inv_alpha = 255 - alpha;

        let r = ((self.r as u32 * alpha + bg.r as u32 * inv_alpha) / 255) as u8;
        let g = ((self.g as u32 * alpha + bg.g as u32 * inv_alpha) / 255) as u8;
        let b = ((self.b as u32 * alpha + bg.b as u32 * inv_alpha) / 255) as u8;

        Color::rgba(r, g, b, 255)
    }

    /// Darkens the color by a specified percentage (0..100).
    pub fn darken(self, percent: u8) -> Color {
        let factor = (100 - percent.min(100)) as u32;
        Color::rgba(
            ((self.r as u32 * factor) / 100) as u8,
            ((self.g as u32 * factor) / 100) as u8,
            ((self.b as u32 * factor) / 100) as u8,
            self.a,
        )
    }

    /// Lightens the color by a specified percentage (0..100).
    pub fn lighten(self, percent: u8) -> Color {
        let pct = percent.min(100) as u32;
        Color::rgba(
            (self.r as u32 + ((255 - self.r as u32) * pct) / 100) as u8,
            (self.g as u32 + ((255 - self.g as u32) * pct) / 100) as u8,
            (self.b as u32 + ((255 - self.b as u32) * pct) / 100) as u8,
            self.a,
        )
    }
}

/// Predefined standard colors and the Micro-OS Light Lavender theme palette.
pub mod theme {
    use super::Color;

    // --- Official User Theme Palette ---
    /// Desktop Background: Light Lavender (#E9DFFF)
    pub const DESKTOP_BG: Color = Color::from_hex(0xE9DFFF);
    /// Primary Purple Accent (#7952A8)
    pub const PRIMARY_PURPLE: Color = Color::from_hex(0x7952A8);
    /// Deep Purple for Title Bars and Active Headers (#5C3C8A)
    pub const DEEP_PURPLE: Color = Color::from_hex(0x5C3C8A);
    /// Window Surface: Crisp Pale Lavender-White (#F8F5FF)
    pub const WINDOW_SURFACE: Color = Color::from_hex(0xF8F5FF);
    /// Primary Text: Dark Plum (#38264F)
    pub const DARK_PLUM_TEXT: Color = Color::from_hex(0x38264F);

    // --- Auxiliary UI Palette ---
    /// Lighter Purple for Hover and Selected Dock Items (#9B72CF)
    pub const HOVER_PURPLE: Color = Color::from_hex(0x9B72CF);
    /// Dock Background: Soft Lavender Surface (#D8C9F8)
    pub const DOCK_BG: Color = Color::from_hex(0xD8C9F8);
    /// Dock Selected Indicator (#BFA0EC)
    pub const DOCK_SELECTED: Color = Color::from_hex(0xBFA0EC);
    /// Window Border Frame (#C8B5E8)
    pub const WINDOW_BORDER: Color = Color::from_hex(0xC8B5E8);
    /// Light Border Accent (#C8B5E8)
    pub const BORDER_LIGHT: Color = Color::from_hex(0xC8B5E8);
    /// Window Drop Shadow (semi-transparent dark plum)
    pub const SHADOW: Color = Color::rgba(0x38, 0x26, 0x4F, 80);

    // --- Title Bar Window Action Buttons ---
    pub const BTN_CLOSE: Color = Color::from_hex(0xE54D42);     // Red
    pub const BTN_MINIMIZE: Color = Color::from_hex(0xF39C12);  // Yellow
    pub const BTN_MAXIMIZE: Color = Color::from_hex(0x2ECC71);  // Green

    // --- Terminal Theme (Deep Dark Plum with Neon Accents) ---
    pub const TERMINAL_BG: Color = Color::from_hex(0x1B1226);
    pub const TERMINAL_TEXT: Color = Color::from_hex(0xE9DFFF);
    pub const TERMINAL_PROMPT: Color = Color::from_hex(0x72D572);
    pub const TERMINAL_CURSOR: Color = Color::from_hex(0xBFA0EC);

    // --- Standard Solid Colors ---
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const LIGHT_GRAY: Color = Color::rgb(200, 200, 200);
    pub const DARK_GRAY: Color = Color::rgb(64, 64, 64);
    pub const GREEN: Color = Color::rgb(46, 204, 113);
    pub const RED: Color = Color::rgb(231, 76, 60);
    pub const BLUE: Color = Color::rgb(52, 152, 219);
    pub const YELLOW: Color = Color::rgb(241, 196, 15);
    pub const CYAN: Color = Color::rgb(26, 188, 156);
}
