//! Files (Virtual File Manager) Application for Desktop GUI.
//!
//! Provides a two-pane directory tree and file viewer interface.

use crate::console::vga::Color;
use crate::gui::compositor::Compositor;
use crate::gui::window::Window;

pub struct FilesApp;

impl FilesApp {
    pub const fn new() -> Self {
        Self
    }

    /// Renders the file manager into the window client area.
    pub fn draw(&self, window: &Window, compositor: &mut Compositor) {
        window.clear_client(compositor);

        let cx = window.client_x();
        let cy = window.client_y();

        // 1. Path Breadcrumb Bar
        compositor.draw_string(cx, cy, "Location: /root/system/  [Up] [New Folder] [Refresh]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx, cy + 1, "----------------------------------------------------------------", Color::LightGray, Color::Black);

        // 2. Left Pane: Directory Tree (width 20)
        let tree_w = 20;
        compositor.draw_box(cx, cy + 2, tree_w, 18, false, Color::Cyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 2, "[ Folders ]", Color::LightCyan, Color::Black);
        compositor.draw_string(cx + 2, cy + 4, "> / (root)", Color::LightGreen, Color::Black);
        compositor.draw_string(cx + 4, cy + 5, "|-- system/", Color::White, Color::Black);
        compositor.draw_string(cx + 4, cy + 6, "|-- network/", Color::White, Color::Black);
        compositor.draw_string(cx + 4, cy + 7, "|-- music/", Color::White, Color::Black);
        compositor.draw_string(cx + 4, cy + 8, "`-- docs/", Color::White, Color::Black);

        // 3. Right Pane: File Listing & Details
        let list_x = cx + tree_w + 1;
        let list_w = window.client_width().saturating_sub(tree_w + 1);
        compositor.draw_box(list_x, cy + 2, list_w, 18, false, Color::Cyan, Color::Black);
        compositor.draw_string(list_x + 2, cy + 2, "[ Contents: /root/system/ ]", Color::LightCyan, Color::Black);

        compositor.draw_string(list_x + 2, cy + 4, "NAME             SIZE      TYPE       MODIFIED", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 5, "------------------------------------------------", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 6, "kernel.bin       2.4 MiB   Executable 2026-11-23", Color::LightGreen, Color::Black);
        compositor.draw_string(list_x + 2, cy + 7, "readme.txt       512 B     Text Doc   2026-11-23", Color::White, Color::Black);
        compositor.draw_string(list_x + 2, cy + 8, "build.info       128 B     Config     2026-11-23", Color::White, Color::Black);
        compositor.draw_string(list_x + 2, cy + 9, "playlist.m3u     1.2 KiB   Audio List 2026-11-23", Color::White, Color::Black);

        // Preview box inside right pane
        compositor.draw_string(list_x + 2, cy + 12, "File Preview (readme.txt):", Color::LightCyan, Color::Black);
        compositor.draw_string(list_x + 2, cy + 13, "\"Welcome to BubbleOS Desktop!\"", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 14, "\"User: maya | Arch: x86_64 Long Mode\"", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 15, "\"Targeting RamFS backing in Phase 2.\"", Color::LightGray, Color::Black);

        // 4. Footer Status
        compositor.draw_string(cx, cy + 21, "RamFS Virtual Storage: 4 items (2.4 MiB) | Free: 118 MiB RAM", Color::LightGray, Color::Black);
    }

    /// Renders the pixel-based file manager into the window client area.
    pub fn draw_pixel(&self, _window: &Window, fb: &mut crate::graphics::Framebuffer) {
        use crate::graphics::{draw, font, theme, Color};

        let cx = Window::CLIENT_PIXEL_X;
        let cy = Window::CLIENT_PIXEL_Y;
        let cw = Window::CLIENT_PIXEL_WIDTH;
        let ch = Window::CLIENT_PIXEL_HEIGHT;

        // Background
        draw::fill_rounded_rect(fb, cx, cy, cw, ch, 4, theme::WINDOW_SURFACE);

        // 1. Breadcrumbs Bar
        let bc_y = cy + 12;
        let bc_h = 44;
        draw::fill_rounded_rect(fb, cx + 16, bc_y, cw - 32, bc_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, bc_y, cw - 32, bc_h, 6, theme::BORDER_LIGHT);

        font::draw_string(fb, cx + 28, bc_y + 14, "Location: /root/system/", theme::DEEP_PURPLE, theme::WHITE);

        let btn_x = cx + cw - 260;
        draw::draw_button(fb, btn_x, bc_y + 6, 60, 32, "Up", false, false);
        draw::draw_button(fb, btn_x + 68, bc_y + 6, 96, 32, "New Folder", false, false);
        draw::draw_button(fb, btn_x + 172, bc_y + 6, 72, 32, "Refresh", false, false);

        // 2. Two-Pane View
        let pane_y = bc_y + bc_h + 12;
        let pane_h = ch.saturating_sub(bc_h + 70);

        // Left Pane: Directory Tree (w = 240)
        let tree_w = 240;
        draw::fill_rounded_rect(fb, cx + 16, pane_y, tree_w, pane_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, cx + 16, pane_y, tree_w, pane_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, cx + 17, pane_y + 1, tree_w - 2, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, cx + 28, pane_y + 7, "Folders & Directories", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, cx + 16, cx + 16 + tree_w, pane_y + 29, theme::BORDER_LIGHT);

        let folders = [
            ("> / (root)", Color::rgb(35, 140, 70), true),
            ("  |-- system/", Color::rgb(56, 38, 79), false),
            ("  |-- network/", Color::rgb(56, 38, 79), false),
            ("  |-- music/", Color::rgb(56, 38, 79), false),
            ("  `-- docs/", Color::rgb(56, 38, 79), false),
        ];
        for (i, (f_name, f_col, f_active)) in folders.iter().enumerate() {
            let row_y = pane_y + 44 + i * 32;
            if *f_active {
                draw::fill_rounded_rect(fb, cx + 24, row_y - 4, tree_w - 16, 28, 4, Color::rgb(240, 235, 252));
            }
            font::draw_string(fb, cx + 32, row_y + 2, f_name, *f_col, if *f_active { Color::rgb(240, 235, 252) } else { theme::WHITE });
        }

        // Right Pane: Contents (w = rest)
        let list_x = cx + 16 + tree_w + 12;
        let list_w = cw.saturating_sub(tree_w + 44);
        draw::fill_rounded_rect(fb, list_x, pane_y, list_w, pane_h, 6, theme::WHITE);
        draw::draw_rounded_rect(fb, list_x, pane_y, list_w, pane_h, 6, theme::BORDER_LIGHT);

        draw::fill_rounded_rect(fb, list_x + 1, pane_y + 1, list_w - 2, 28, 5, Color::rgb(245, 240, 255));
        font::draw_string(fb, list_x + 16, pane_y + 7, "Contents: /root/system/", theme::PRIMARY_PURPLE, Color::rgb(245, 240, 255));
        draw::draw_hline(fb, list_x, list_x + list_w, pane_y + 29, theme::BORDER_LIGHT);

        // Table Header
        font::draw_string(fb, list_x + 16, pane_y + 40, "NAME               SIZE        TYPE           MODIFIED", Color::rgb(120, 105, 140), theme::WHITE);
        draw::draw_hline(fb, list_x + 16, list_x + list_w - 16, pane_y + 60, theme::BORDER_LIGHT);

        let files = [
            ("kernel.bin", "2.4 MiB", "Executable", "2026-11-23", Color::rgb(35, 140, 70)),
            ("readme.txt", "512 B", "Text Doc", "2026-11-23", Color::rgb(56, 38, 79)),
            ("build.info", "128 B", "Config", "2026-11-23", Color::rgb(56, 38, 79)),
            ("mayiiiii.mp3", "1.8 MiB", "Audio File", "2026-11-23", Color::rgb(130, 85, 210)),
            ("playlist.m3u", "1.2 KiB", "Playlist", "2026-11-23", Color::rgb(56, 38, 79)),
        ];

        for (i, (fname, fsize, ftype, fmod, fcolor)) in files.iter().enumerate() {
            let row_y = pane_y + 72 + i * 32;
            font::draw_string(fb, list_x + 16, row_y, fname, *fcolor, theme::WHITE);
            font::draw_string(fb, list_x + 160, row_y, fsize, Color::rgb(100, 90, 120), theme::WHITE);
            font::draw_string(fb, list_x + 256, row_y, ftype, Color::rgb(100, 90, 120), theme::WHITE);
            font::draw_string(fb, list_x + 380, row_y, fmod, Color::rgb(130, 120, 150), theme::WHITE);
        }

        // Preview box inside right pane
        let prev_y = pane_y + 240;
        let prev_h = pane_h.saturating_sub(256);
        draw::fill_rounded_rect(fb, list_x + 16, prev_y, list_w - 32, prev_h, 6, Color::rgb(248, 245, 255));
        draw::draw_rounded_rect(fb, list_x + 16, prev_y, list_w - 32, prev_h, 6, theme::BORDER_LIGHT);

        font::draw_string(fb, list_x + 28, prev_y + 12, "File Preview: readme.txt", theme::PRIMARY_PURPLE, Color::rgb(248, 245, 255));
        font::draw_string(fb, list_x + 28, prev_y + 36, "\"Welcome to BubbleOS Desktop!\"", theme::DARK_PLUM_TEXT, Color::rgb(248, 245, 255));
        font::draw_string(fb, list_x + 28, prev_y + 56, "\"User: maya | Arch: x86_64 Long Mode | VRAM: 1024x768x32 BGA\"", theme::DARK_PLUM_TEXT, Color::rgb(248, 245, 255));
        font::draw_string(fb, list_x + 28, prev_y + 76, "\"RamFS virtual file system backing fully operational.\"", Color::rgb(120, 105, 140), Color::rgb(248, 245, 255));

        // 3. Status Bar
        let sb_y = pane_y + pane_h + 12;
        draw::fill_rounded_rect(fb, cx + 16, sb_y, cw - 32, 32, 4, Color::rgb(245, 240, 255));
        draw::draw_rounded_rect(fb, cx + 16, sb_y, cw - 32, 32, 4, theme::BORDER_LIGHT);
        font::draw_string(fb, cx + 28, sb_y + 8, "Status: Ready | 5 files, 4 folders | 4.2 MiB total storage", Color::rgb(40, 150, 70), Color::rgb(245, 240, 255));
    }
}
