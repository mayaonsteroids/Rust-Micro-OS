//! Hardware Mouse Cursor Rendering and Restoration for BubbleOS.
//!
//! Provides pixel-perfect classic arrow pointer rendering directly to the visible front buffer,
//! with instantaneous background restoration from the clean off-screen back buffer.
//! This ensures zero flicker, zero tearing, and zero overhead full-screen redraws.

use crate::graphics::Framebuffer;

/// Dimensions of the classic arrow cursor bitmap.
pub const CURSOR_WIDTH: usize = 12;
pub const CURSOR_HEIGHT: usize = 19;

/// 0 = Transparent, 1 = Black Outline (0xFF000000), 2 = White Fill (0xFFFFFFFF)
pub const CURSOR_PATTERN: [[u8; CURSOR_WIDTH]; CURSOR_HEIGHT] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
    [1, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 0],
    [1, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0, 0],
    [1, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0],
];

/// Restores the pristine background pixels from `back_ptr` to `front_ptr`
/// over the `CURSOR_WIDTH x CURSOR_HEIGHT` region at `(x, y)`.
pub fn erase_cursor(fb: &mut Framebuffer, x: usize, y: usize) {
    if fb.front_ptr.is_null() || fb.back_ptr.is_null() {
        return;
    }

    let w = fb.width;
    let h = fb.height;

    for r in 0..CURSOR_HEIGHT {
        let py = y + r;
        if py >= h {
            break;
        }
        let row_offset = py * w;
        for c in 0..CURSOR_WIDTH {
            let px = x + c;
            if px >= w {
                break;
            }
            let idx = row_offset + px;
            unsafe {
                *fb.front_ptr.add(idx) = *fb.back_ptr.add(idx);
            }
        }
    }
}

/// Renders the classic arrow pointer directly onto the visible `front_ptr` at `(x, y)`.
pub fn draw_cursor(fb: &mut Framebuffer, x: usize, y: usize) {
    if fb.front_ptr.is_null() {
        return;
    }

    let w = fb.width;
    let h = fb.height;

    for r in 0..CURSOR_HEIGHT {
        let py = y + r;
        if py >= h {
            break;
        }
        let row_offset = py * w;
        for c in 0..CURSOR_WIDTH {
            let px = x + c;
            if px >= w {
                break;
            }
            let code = CURSOR_PATTERN[r][c];
            if code == 1 {
                // Black outline
                unsafe {
                    *fb.front_ptr.add(row_offset + px) = 0xFF00_0000;
                }
            } else if code == 2 {
                // White fill
                unsafe {
                    *fb.front_ptr.add(row_offset + px) = 0xFFFF_FFFF;
                }
            }
        }
    }
}

/// Updates the visible cursor position: erases at `(old_x, old_y)` and renders at `(new_x, new_y)`.
pub fn update_cursor_position(
    fb: &mut Framebuffer,
    old_x: usize,
    old_y: usize,
    new_x: usize,
    new_y: usize,
) {
    if old_x != new_x || old_y != new_y {
        erase_cursor(fb, old_x, old_y);
    }
    draw_cursor(fb, new_x, new_y);
}
