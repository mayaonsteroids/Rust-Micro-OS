//! High-Level 2D Graphics Drawing Primitives.
//!
//! Provides geometric rendering: rectangles, borders, lines, rounded corners,
//! drop shadows, bevels, and themed window components.

use crate::graphics::color::{theme, Color};
use crate::graphics::framebuffer::Framebuffer;
use crate::graphics::font;

/// Draws a 1-pixel rectangle outline.
pub fn draw_rect(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, color: Color) {
    if w == 0 || h == 0 {
        return;
    }
    // Top and bottom borders
    draw_hline(fb, x, y, w, color);
    draw_hline(fb, x, y + h.saturating_sub(1), w, color);
    // Left and right borders
    draw_vline(fb, x, y, h, color);
    draw_vline(fb, x + w.saturating_sub(1), y, h, color);
}

/// Fills a solid rectangle.
#[inline]
pub fn fill_rect(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, color: Color) {
    fb.fill_rect(x, y, w, h, color);
}

/// Draws a horizontal line of `len` pixels.
#[inline]
pub fn draw_hline(fb: &mut Framebuffer, x: usize, y: usize, len: usize, color: Color) {
    fb.fill_rect(x, y, len, 1, color);
}

/// Draws a vertical line of `len` pixels.
#[inline]
pub fn draw_vline(fb: &mut Framebuffer, x: usize, y: usize, len: usize, color: Color) {
    fb.fill_rect(x, y, 1, len, color);
}

/// Draws a line between two points using Bresenham's algorithm.
pub fn draw_line(fb: &mut Framebuffer, mut x0: isize, mut y0: isize, x1: isize, y1: isize, color: Color) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && y0 >= 0 {
            fb.set_pixel(x0 as usize, y0 as usize, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Draws a rounded rectangle outline with corner radius `r`.
pub fn draw_rounded_rect(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, r: usize, color: Color) {
    let r = r.min(w / 2).min(h / 2);
    if r <= 1 {
        draw_rect(fb, x, y, w, h, color);
        return;
    }

    // Straight edges
    draw_hline(fb, x + r, y, w - 2 * r, color);
    draw_hline(fb, x + r, y + h - 1, w - 2 * r, color);
    draw_vline(fb, x, y + r, h - 2 * r, color);
    draw_vline(fb, x + w - 1, y + r, h - 2 * r, color);

    // 4 Corner arcs using midpoint circle logic
    let mut f = 1 - r as isize;
    let mut dd_f_x = 1;
    let mut dd_f_y = -2 * (r as isize);
    let mut cx = 0isize;
    let mut cy = r as isize;

    while cx < cy {
        if f >= 0 {
            cy -= 1;
            dd_f_y += 2;
            f += dd_f_y;
        }
        cx += 1;
        dd_f_x += 2;
        f += dd_f_x;

        // Top-Left
        fb.set_pixel((x + r - cx as usize).max(0), (y + r - cy as usize).max(0), color);
        fb.set_pixel((x + r - cy as usize).max(0), (y + r - cx as usize).max(0), color);
        // Top-Right
        fb.set_pixel(x + w - 1 - r + cx as usize, (y + r - cy as usize).max(0), color);
        fb.set_pixel(x + w - 1 - r + cy as usize, (y + r - cx as usize).max(0), color);
        // Bottom-Left
        fb.set_pixel((x + r - cx as usize).max(0), y + h - 1 - r + cy as usize, color);
        fb.set_pixel((x + r - cy as usize).max(0), y + h - 1 - r + cx as usize, color);
        // Bottom-Right
        fb.set_pixel(x + w - 1 - r + cx as usize, y + h - 1 - r + cy as usize, color);
        fb.set_pixel(x + w - 1 - r + cy as usize, y + h - 1 - r + cx as usize, color);
    }
}

/// Fills a rounded rectangle with corner radius `r`.
pub fn fill_rounded_rect(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, r: usize, color: Color) {
    let r = r.min(w / 2).min(h / 2);
    if r <= 1 {
        fill_rect(fb, x, y, w, h, color);
        return;
    }

    // Main center box
    fill_rect(fb, x + r, y, w - 2 * r, h, color);
    // Left & Right side boxes
    fill_rect(fb, x, y + r, r, h - 2 * r, color);
    fill_rect(fb, x + w - r, y + r, r, h - 2 * r, color);

    // Corner fills
    let mut cx = 0isize;
    let mut cy = r as isize;
    let mut d = 3 - 2 * (r as isize);

    while cx <= cy {
        // Horizontal lines between arc points
        draw_hline(fb, x + r - cy as usize, y + r - cx as usize, (cy as usize - cx as usize) + 1, color);
        draw_hline(fb, x + r - cx as usize, y + r - cy as usize, (cx as usize - 0) + 1, color);

        draw_hline(fb, x + w - r + cx as usize, y + r - cy as usize, (cy as usize - cx as usize) + 1, color);
        draw_hline(fb, x + w - r + cx as usize, y + r - cx as usize, (cy as usize - cx as usize) + 1, color);

        draw_hline(fb, x + r - cy as usize, y + h - r + cx as usize, (cy as usize - cx as usize) + 1, color);
        draw_hline(fb, x + r - cx as usize, y + h - r + cy as usize, (cx as usize - 0) + 1, color);

        draw_hline(fb, x + w - r + cx as usize, y + h - r + cy as usize, (cy as usize - cx as usize) + 1, color);
        draw_hline(fb, x + w - r + cx as usize, y + h - r + cx as usize, (cy as usize - cx as usize) + 1, color);

        if d < 0 {
            d += 4 * cx + 6;
        } else {
            d += 4 * (cx - cy) + 10;
            cy -= 1;
        }
        cx += 1;
    }
}

/// Renders a soft window drop shadow offset to the bottom-right.
pub fn draw_shadow(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, offset: usize) {
    let shadow_color = theme::SHADOW;
    // Bottom shadow
    fill_rect(fb, x + offset, y + h, w, offset, shadow_color);
    // Right shadow
    fill_rect(fb, x + w, y + offset, offset, h + offset, shadow_color);
}

/// Renders an interactive desktop button with border, background, and centered text label.
pub fn draw_button(
    fb: &mut Framebuffer,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    label: &str,
    is_pressed: bool,
    is_focused: bool,
) {
    let bg_color = if is_pressed {
        theme::HOVER_PURPLE
    } else if is_focused {
        theme::WINDOW_SURFACE.darken(6)
    } else {
        theme::WINDOW_SURFACE
    };

    let border_color = if is_focused {
        theme::PRIMARY_PURPLE
    } else {
        theme::WINDOW_BORDER
    };

    let text_color = if is_pressed {
        theme::WHITE
    } else {
        theme::DARK_PLUM_TEXT
    };

    // Draw background & border
    fill_rounded_rect(fb, x, y, w, h, 4, bg_color);
    draw_rounded_rect(fb, x, y, w, h, 4, border_color);

    // Center text label inside button
    let text_width = label.len() * font::FONT_WIDTH;
    let text_x = x + (w.saturating_sub(text_width)) / 2;
    let text_y = y + (h.saturating_sub(font::FONT_HEIGHT)) / 2;
    font::draw_string(fb, text_x, text_y, label, text_color, None);
}
