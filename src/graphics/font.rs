//! 8×16 Bitmap Font and Text Rendering Subsystem.
//!
//! Provides proportional and monospaced text rendering, scaled rendering (1x, 2x),
//! transparent and opaque backgrounds, and bounds clipping.

use crate::graphics::color::Color;
use crate::graphics::framebuffer::Framebuffer;

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 16;

/// Embeds standard 8x16 font glyphs (16 bytes per character).
pub mod glyphs {
    include!("font_data.rs");
}

/// Renders a single 8x16 character at `(x, y)` on the framebuffer.
pub fn draw_char(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    ch: u8,
    fg: Color,
    bg: Option<Color>,
) {
    let glyph = glyphs::get_glyph(ch);

    for row in 0..16 {
        let py = y + row;
        let bits = glyph[row];

        for col in 0..8 {
            let px = x + col;
            let bit_set = (bits & (0x80 >> col)) != 0;

            if bit_set {
                fb.set_pixel(px, py, fg);
            } else if let Some(bg_color) = bg {
                fb.set_pixel(px, py, bg_color);
            }
        }
    }
}

/// Renders a single character scaled by an integer factor (e.g. 2x = 16x32).
pub fn draw_char_scaled(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    ch: u8,
    scale: usize,
    fg: Color,
    bg: Option<Color>,
) {
    if scale <= 1 {
        draw_char(fb, x, y, ch, fg, bg);
        return;
    }

    let glyph = glyphs::get_glyph(ch);

    for row in 0..16 {
        let bits = glyph[row];
        for col in 0..8 {
            let bit_set = (bits & (0x80 >> col)) != 0;
            let target_color = if bit_set { fg } else { match bg { Some(c) => c, None => continue } };

            let px = x + col * scale;
            let py = y + row * scale;
            fb.fill_rect(px, py, scale, scale, target_color);
        }
    }
}

/// Renders an ASCII text string at `(x, y)`. Returns the x-coordinate after the string.
pub fn draw_string(
    fb: &mut Framebuffer,
    mut x: usize,
    y: usize,
    s: &str,
    fg: Color,
    bg: impl Into<Option<Color>>,
) -> usize {
    let bg_opt = bg.into();
    for b in s.bytes() {
        if b == b'\n' {
            break;
        }
        draw_char(fb, x, y, b, fg, bg_opt);
        x += FONT_WIDTH;
    }
    x
}

/// Renders an ASCII text string scaled by an integer factor with transparent background.
pub fn draw_string_scaled(
    fb: &mut Framebuffer,
    mut x: usize,
    y: usize,
    s: &str,
    fg: Color,
    scale: usize,
) -> usize {
    let char_w = FONT_WIDTH * scale;
    for b in s.bytes() {
        if b == b'\n' {
            break;
        }
        draw_char_scaled(fb, x, y, b, scale, fg, None);
        x += char_w;
    }
    x
}
