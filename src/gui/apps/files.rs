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
        compositor.draw_string(list_x + 2, cy + 13, "\"Welcome to Rust Micro-OS Desktop!\"", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 14, "\"User: maya | Arch: x86_64 Long Mode\"", Color::LightGray, Color::Black);
        compositor.draw_string(list_x + 2, cy + 15, "\"Targeting RamFS backing in Phase 2.\"", Color::LightGray, Color::Black);

        // 4. Footer Status
        compositor.draw_string(cx, cy + 21, "RamFS Virtual Storage: 4 items (2.4 MiB) | Free: 118 MiB RAM", Color::LightGray, Color::Black);
    }
}
