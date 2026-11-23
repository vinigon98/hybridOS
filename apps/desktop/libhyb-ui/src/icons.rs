//! Vector-like Pixel Icon Rasterizer for Shell & Desktop Apps

use crate::surface::Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconType {
    Folder,
    File,
    Terminal,
    Search,
    Gear,
    Minimize,
    Maximize,
    Close,
    Drive,
    Trash,
    Code,
    Image,
    Lock,
    Calculator,
}

/// Render a clean 16x16 icon onto the surface at (x, y)
pub fn draw_icon(surface: &mut Surface, x: usize, y: usize, icon: IconType, color: u32) {
    let ix = x as i32;
    let iy = y as i32;

    match icon {
        IconType::Folder => {
            // Tab
            surface.fill_rect(ix + 1, iy + 2, 6, 2, color);
            // Body
            surface.fill_rect(ix + 1, iy + 4, 14, 10, color);
            surface.fill_rect(ix + 2, iy + 5, 12, 8, 0xFFE0B040); // Amber inside
        }
        IconType::File => {
            surface.fill_rect(ix + 3, iy + 2, 10, 12, color);
            surface.fill_rect(ix + 4, iy + 3, 8, 10, 0xFFFFFFFF);
            // File lines
            surface.fill_rect(ix + 5, iy + 5, 6, 1, 0xFF888888);
            surface.fill_rect(ix + 5, iy + 7, 6, 1, 0xFF888888);
            surface.fill_rect(ix + 5, iy + 9, 4, 1, 0xFF888888);
        }
        IconType::Terminal => {
            surface.fill_rect(ix + 1, iy + 2, 14, 12, 0xFF202020);
            surface.draw_rect(ix + 1, iy + 2, 14, 12, color);
            // Prompt >
            surface.draw_line(ix + 3, iy + 5, ix + 5, iy + 7, color);
            surface.draw_line(ix + 5, iy + 7, ix + 3, iy + 9, color);
            // Cursor _
            surface.fill_rect(ix + 7, iy + 9, 4, 1, color);
        }
        IconType::Search => {
            // Glass circle
            surface.draw_rect(ix + 3, iy + 3, 8, 8, color);
            // Handle
            surface.draw_line(ix + 10, iy + 10, ix + 14, iy + 14, color);
            surface.draw_line(ix + 11, iy + 10, ix + 14, iy + 13, color);
        }
        IconType::Gear => {
            surface.draw_rect(ix + 4, iy + 4, 8, 8, color);
            surface.fill_rect(ix + 7, iy + 1, 2, 14, color);
            surface.fill_rect(ix + 1, iy + 7, 14, 2, color);
        }
        IconType::Minimize => {
            surface.fill_rect(ix + 2, iy + 8, 12, 2, color);
        }
        IconType::Maximize => {
            surface.draw_rect(ix + 2, iy + 2, 12, 12, color);
            surface.fill_rect(ix + 2, iy + 2, 12, 3, color);
        }
        IconType::Close => {
            surface.draw_line(ix + 3, iy + 3, ix + 13, iy + 13, color);
            surface.draw_line(ix + 13, iy + 3, ix + 3, iy + 13, color);
            surface.draw_line(ix + 4, iy + 3, ix + 13, iy + 12, color);
            surface.draw_line(ix + 12, iy + 3, ix + 3, iy + 12, color);
        }
        IconType::Drive => {
            surface.fill_rect(ix + 2, iy + 4, 12, 8, color);
            surface.fill_rect(ix + 10, iy + 8, 2, 2, 0xFF50FA7B); // Activity LED
        }
        IconType::Trash => {
            surface.fill_rect(ix + 3, iy + 2, 10, 2, color);
            surface.fill_rect(ix + 4, iy + 4, 8, 10, color);
            surface.fill_rect(ix + 6, iy + 6, 1, 6, 0xFFFFFFFF);
            surface.fill_rect(ix + 9, iy + 6, 1, 6, 0xFFFFFFFF);
        }
        IconType::Code => {
            surface.draw_line(ix + 5, iy + 5, ix + 2, iy + 8, color);
            surface.draw_line(ix + 2, iy + 8, ix + 5, iy + 11, color);
            surface.draw_line(ix + 10, iy + 5, ix + 13, iy + 8, color);
            surface.draw_line(ix + 13, iy + 8, ix + 10, iy + 11, color);
            surface.draw_line(ix + 9, iy + 4, ix + 6, iy + 12, 0xFF50FA7B);
        }
        IconType::Image => {
            surface.draw_rect(ix + 2, iy + 2, 12, 12, color);
            surface.fill_rect(ix + 4, iy + 4, 3, 3, 0xFFFFFF00); // Sun
            // Mountain
            surface.draw_line(ix + 3, iy + 12, ix + 7, iy + 8, color);
            surface.draw_line(ix + 7, iy + 8, ix + 12, iy + 12, color);
        }
        IconType::Lock => {
            surface.draw_rect(ix + 5, iy + 2, 6, 6, color);
            surface.fill_rect(ix + 3, iy + 7, 10, 8, color);
            surface.fill_rect(ix + 7, iy + 10, 2, 3, 0xFFFFFFFF);
        }
        IconType::Calculator => {
            surface.draw_rect(ix + 3, iy + 1, 10, 14, color);
            surface.fill_rect(ix + 5, iy + 3, 6, 2, 0xFF50FA7B); // Screen
            surface.fill_rect(ix + 5, iy + 7, 2, 2, color);
            surface.fill_rect(ix + 9, iy + 7, 2, 2, color);
            surface.fill_rect(ix + 5, iy + 11, 2, 2, color);
            surface.fill_rect(ix + 9, iy + 11, 2, 2, color);
        }
    }
}
