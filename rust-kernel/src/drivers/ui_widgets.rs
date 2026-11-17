//! HybridOS Crystal Glass UI — High-Performance Immediate-Mode Widget Engine (IMGUI)
//!
//! Provides interactive, photorealistic graphical primitives for all 50 native HybridOS applications:
//! 1. `ButtonStyle` & `ui_glass_button`: Hover/click physics, 14% white specular sheen, 1px chamfer, 3D offset.
//! 2. `ui_tab_bar`: Horizontal tabs with 4px top rounded corners, active sapphire accent, translucent idle state.
//! 3. `ui_text_input`: White rectangular box, blue translucent border, horizontal scrolling, blinking PIT cursor, PS/2 input.
//! 4. `ui_table_view`: Ice-blue headers, alternating zebra striping (#FFFFFF / #F8FAFC), row selection, vitrified scrollbar.
//! 5. `ui_progress_bar`: Chamfered glass frame, liquid gradient fill (Emerald/Sapphire), ruby alert >= 90%.
//! 6. `ui_slider`: 4px recessed track, 14px circular glass knob, proportional drag calculation, min..max clamping.
//! 7. `ui_checkbox` & `ui_radio_button`: 14x14px rounded box, white background, sapphire checkmark / dot.
//! 8. `ui_breadcrumbs_bar`: Interactive segmented path bar with hover glow, chevron separators, and VFS navigation.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use crate::arch::idt::SYSTEM_TICKS;
use crate::drivers::gfx2d::*;
use crate::drivers::gop::FONT_8X16;
use crate::drivers::pop_keyboard_char;

// =============================================================================
// COLOR PALETTE — HYBRIDOS CRYSTAL GLASS SPECIFICATION
// =============================================================================

pub const COLOR_SAPPHIRE_PRIMARY: u32 = 0x000284C7; // Blue 600
pub const COLOR_SAPPHIRE_HOVER: u32   = 0x0038BDF8; // Sky 400
pub const COLOR_SAPPHIRE_BORDER: u32  = 0x007DD3FC; // Sky 300
pub const COLOR_SAPPHIRE_DARK: u32    = 0x000369A1; // Blue 700
pub const COLOR_SAPPHIRE_DEEP: u32    = 0x000C4A6E; // Blue 900

pub const COLOR_ICE_BLUE_TOP: u32     = 0x00EBF3FC; // Ice blue highlight
pub const COLOR_ICE_BLUE_BOT: u32     = 0x00D9E7F8; // Ice blue base
pub const COLOR_ICE_WHITE: u32        = 0x00FFFFFF; // Pure white
pub const COLOR_ICE_LIGHT_GRAY: u32   = 0x00F8FAFC; // Slate 50 (zebra stripe)
pub const COLOR_ICE_HOVER_GLOW: u32   = 0x00E0F2FE; // Light Sky Glow
pub const COLOR_ICE_SELECTED_BG: u32  = 0x00BAE6FD; // Sky 200

pub const COLOR_RUBY_ALERT_TOP: u32   = 0x00EF4444; // Red 500
pub const COLOR_RUBY_ALERT_BOT: u32   = 0x00B91C1C; // Red 700
pub const COLOR_RUBY_HOVER_TOP: u32   = 0x00F87171; // Red 400
pub const COLOR_RUBY_HOVER_BOT: u32   = 0x00DC2626; // Red 600
pub const COLOR_RUBY_BORDER: u32      = 0x00FECACA; // Red 200

pub const COLOR_EMERALD_TOP: u32      = 0x0034D399; // Emerald 400
pub const COLOR_EMERALD_BOT: u32      = 0x0010B981; // Emerald 500
pub const COLOR_EMERALD_BORDER: u32   = 0x00A7F3D0; // Emerald 200

pub const COLOR_BORDER_SUBTLE: u32    = 0x00CBD5E1; // Slate 300
pub const COLOR_BORDER_STRONG: u32    = 0x0094A3B8; // Slate 400
pub const COLOR_TEXT_PRIMARY: u32     = 0x000F172A; // Slate 900
pub const COLOR_TEXT_SECONDARY: u32   = 0x00475569; // Slate 600
pub const COLOR_TEXT_MUTED: u32       = 0x0064748B; // Slate 500

// =============================================================================
// TYPOGRAPHY & FONT RENDERING HELPERS
// =============================================================================

#[inline]
pub fn map_char_to_ascii(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'Á' | 'À' | 'Ã' | 'Â' | 'Ä' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        '•' => '*',
        '—' | '–' => '-',
        '“' | '”' | '«' | '»' => '"',
        '‘' | '’' => '\'',
        '▸' | '►' => '>',
        '◄' => '<',
        '▲' => '^',
        '▼' => 'v',
        '✔' | '✓' => 'v',
        '✕' | '✖' => 'x',
        c if (c as u32) >= 32 && (c as u32) <= 126 => c,
        _ => '?',
    }
}

/// Plots an ASCII string with clipping against horizontal coordinate `max_x`.
pub unsafe fn draw_text_clipped(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    max_x: usize,
    text: &str,
    fg: u32,
    bg: Option<u32>,
) {
    if y + 16 > scr_h || x >= scr_w || fb.is_null() {
        return;
    }
    let limit_x = max_x.min(scr_w);
    for (i, raw_ch) in text.chars().enumerate() {
        let cx = x + i * 8;
        if cx + 8 > limit_x {
            break;
        }
        let ch = map_char_to_ascii(raw_ch);
        let ascii = ch as usize;
        let font_idx = (ascii.saturating_sub(32)) * 16;
        for row in 0..16 {
            let py = y + row;
            if py >= scr_h {
                break;
            }
            let bits = if font_idx + row < FONT_8X16.len() {
                FONT_8X16[font_idx + row]
            } else {
                0
            };
            let row_ptr = fb.add(py * pitch) as *mut u32;
            for col in 0..8 {
                let px = cx + col;
                if px < limit_x {
                    if (bits & (0x80 >> col)) != 0 {
                        *row_ptr.add(px) = fg;
                    } else if let Some(bg_col) = bg {
                        *row_ptr.add(px) = bg_col;
                    }
                }
            }
        }
    }
}

/// Plots text centered horizontally within bounding box `[x, x + w]`.
pub unsafe fn draw_text_centered(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    text: &str,
    fg: u32,
) {
    let text_w = text.chars().count() * 8;
    let tx = x + (w.saturating_sub(text_w)) / 2;
    let ty = y + (h.saturating_sub(16)) / 2;
    draw_text_clipped(fb, pitch, scr_w, scr_h, tx, ty, x + w, text, fg, None);
}

// =============================================================================
// 2D SPECIALIZED SHAPES & SPECULAR PRIMITIVES
// =============================================================================

/// Fills a rectangle whose top two corners are rounded with `radius`, and bottom two corners are square.
pub unsafe fn fill_top_rounded_rect(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    color: u32,
) {
    if w == 0 || h == 0 || x >= scr_w || y >= scr_h || fb.is_null() {
        return;
    }
    let r = radius.min(w / 2).min(h);
    let x_end = (x + w).min(scr_w);
    let y_end = (y + h).min(scr_h);
    let r_sq = (r * r) as isize;

    for py in y..y_end {
        let rel_y = py - y;
        let (row_x_start, row_x_end) = if rel_y < r && r > 0 {
            let dy = (r - rel_y) as isize;
            let dx_in = isqrt((r_sq - dy * dy).max(0) as usize);
            (x + r - dx_in, x + w - r + dx_in)
        } else {
            (x, x + w)
        };

        let span_start = row_x_start.max(x).min(scr_w);
        let span_end = row_x_end.min(x_end);

        if span_start < span_end {
            let row = fb.add(py * pitch) as *mut u32;
            for px in span_start..span_end {
                *row.add(px) = color;
            }
        }
    }
}

/// Draws an Aero Glass 14% white specular sheen across the upper crescent of a control.
pub unsafe fn apply_specular_sheen(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    radius: usize,
    max_alpha: u8,
) {
    if w == 0 || h < 4 || x >= scr_w || y >= scr_h || fb.is_null() {
        return;
    }
    let sheen_h = (h * 45) / 100;
    let r = radius.min(w / 2).min(h / 2);
    let r_sq = (r * r) as isize;
    let x_end = (x + w).min(scr_w);

    for sy in 0..sheen_h {
        let py = y + sy;
        if py >= scr_h {
            break;
        }
        let alpha = ((sheen_h - sy) * (max_alpha as usize) / sheen_h).clamp(0, 255) as u8;
        if alpha == 0 {
            continue;
        }

        let (row_x_start, row_x_end) = if sy < r && r > 0 {
            let dy = (r - sy) as isize;
            let dx_in = isqrt((r_sq - dy * dy).max(0) as usize);
            (x + r - dx_in, x + w - r + dx_in)
        } else {
            (x, x + w)
        };

        let span_start = row_x_start.max(x).min(scr_w);
        let span_end = row_x_end.min(x_end);

        if span_start < span_end {
            let row = fb.add(py * pitch) as *mut u32;
            for px in span_start..span_end {
                let cur = *row.add(px);
                *row.add(px) = blend_argb(cur, COLOR_ICE_WHITE, alpha);
            }
        }
    }
}

/// Draws a 1px chamfer (top-highlight and bottom-shadow) giving convex 3D vitrified depth.
pub unsafe fn apply_bevel_chamfer(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    pressed: bool,
) {
    if w < 4 || h < 4 || x >= scr_w || y >= scr_h || fb.is_null() {
        return;
    }
    let (top_col, top_alpha, bot_col, bot_alpha) = if pressed {
        // Inverted depth when clicked down
        (0x00000000, 40u8, COLOR_ICE_WHITE, 90u8)
    } else {
        (COLOR_ICE_WHITE, 110u8, 0x00000000, 45u8)
    };

    // 1px Top highlight bevel
    let top_y = y + 1;
    if top_y < scr_h {
        let row = fb.add(top_y * pitch) as *mut u32;
        let x_start = (x + 2).min(scr_w);
        let x_end = (x + w.saturating_sub(2)).min(scr_w);
        for px in x_start..x_end {
            let cur = *row.add(px);
            *row.add(px) = blend_argb(cur, top_col, top_alpha);
        }
    }

    // 1px Bottom shadow bevel
    let bot_y = y + h.saturating_sub(2);
    if bot_y < scr_h && bot_y > top_y {
        let row = fb.add(bot_y * pitch) as *mut u32;
        let x_start = (x + 2).min(scr_w);
        let x_end = (x + w.saturating_sub(2)).min(scr_w);
        for px in x_start..x_end {
            let cur = *row.add(px);
            *row.add(px) = blend_argb(cur, bot_col, bot_alpha);
        }
    }
}

// =============================================================================
// 1. BUTTON STYLE ENUM & 2. IMMEDIATE-MODE GLASS BUTTON
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStyle {
    StandardGlass, // Convex Ice-Blue
    PrimaryAction, // Luminous Sapphire Blue
    DangerRuby,    // Vitrified Ruby Red
    ToolbarFlat,   // Transparent chassis, glowing hover
}

/// 2. Immediate-Mode Crystal Glass Button with hover/click detection,
/// 14% white specular sheen, 1px chamfer, centered typography, and 1px 3D offset when pressed.
/// Retorna `true` se clicado.
pub unsafe fn ui_glass_button(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    label: &str,
    style: ButtonStyle,
    mx: i32,
    my: i32,
    clicked: bool,
) -> bool {
    let is_hover = mx >= x as i32 && mx < (x + w) as i32 && my >= y as i32 && my < (y + h) as i32;
    let is_pressed = is_hover && clicked;

    let (c_top, c_bot, border_col, text_col, sheen_alpha) = match style {
        ButtonStyle::PrimaryAction => {
            if is_pressed {
                (COLOR_SAPPHIRE_DARK, COLOR_SAPPHIRE_DEEP, COLOR_SAPPHIRE_BORDER, COLOR_ICE_WHITE, 20u8)
            } else if is_hover {
                (COLOR_SAPPHIRE_HOVER, COLOR_SAPPHIRE_PRIMARY, COLOR_ICE_WHITE, COLOR_ICE_WHITE, 45u8)
            } else {
                (COLOR_SAPPHIRE_PRIMARY, COLOR_SAPPHIRE_DARK, COLOR_SAPPHIRE_BORDER, COLOR_ICE_WHITE, 36u8)
            }
        }
        ButtonStyle::DangerRuby => {
            if is_pressed {
                (COLOR_RUBY_ALERT_BOT, 0x00991B1B, COLOR_RUBY_BORDER, COLOR_ICE_WHITE, 20u8)
            } else if is_hover {
                (COLOR_RUBY_HOVER_TOP, COLOR_RUBY_HOVER_BOT, COLOR_ICE_WHITE, COLOR_ICE_WHITE, 45u8)
            } else {
                (COLOR_RUBY_ALERT_TOP, COLOR_RUBY_ALERT_BOT, COLOR_RUBY_BORDER, COLOR_ICE_WHITE, 36u8)
            }
        }
        ButtonStyle::ToolbarFlat => {
            if is_pressed {
                (COLOR_ICE_SELECTED_BG, COLOR_SAPPHIRE_HOVER, COLOR_SAPPHIRE_PRIMARY, COLOR_SAPPHIRE_PRIMARY, 20u8)
            } else if is_hover {
                (COLOR_ICE_HOVER_GLOW, COLOR_ICE_SELECTED_BG, COLOR_SAPPHIRE_HOVER, COLOR_SAPPHIRE_PRIMARY, 36u8)
            } else {
                (0x00000000, 0x00000000, 0x00000000, COLOR_TEXT_SECONDARY, 0u8)
            }
        }
        ButtonStyle::StandardGlass => {
            if is_pressed {
                (COLOR_BORDER_SUBTLE, COLOR_ICE_WHITE, COLOR_SAPPHIRE_PRIMARY, COLOR_SAPPHIRE_PRIMARY, 15u8)
            } else if is_hover {
                (COLOR_ICE_WHITE, COLOR_ICE_HOVER_GLOW, COLOR_SAPPHIRE_BORDER, COLOR_SAPPHIRE_PRIMARY, 40u8)
            } else {
                (COLOR_ICE_WHITE, COLOR_BORDER_SUBTLE, COLOR_BORDER_STRONG, COLOR_TEXT_PRIMARY, 36u8)
            }
        }
    };

    // 1. Draw button background chassis
    if style != ButtonStyle::ToolbarFlat || is_hover || is_pressed {
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, x, y, w, h, 4, c_top, c_bot);

        // 2. Draw 14% specular white reflection sheen on upper crescent (~36/255 ≈ 14.1%)
        if sheen_alpha > 0 {
            apply_specular_sheen(fb, pitch, scr_w, scr_h, x, y, w, h, 4, sheen_alpha);
        }

        // 3. Draw 1px top-highlight and bottom-shadow chamfer bevel
        apply_bevel_chamfer(fb, pitch, scr_w, scr_h, x, y, w, h, is_pressed);

        // 4. Draw outer border
        if border_col != 0 {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, w, h, 4, border_col);
        }
    }

    // 5. Centered Typography with 1px 3D offset when pressed
    let text_w = label.chars().count() * 8;
    let base_tx = x + (w.saturating_sub(text_w)) / 2;
    let base_ty = y + (h.saturating_sub(16)) / 2;
    let (tx, ty) = if is_pressed {
        (base_tx + 1, base_ty + 1)
    } else {
        (base_tx, base_ty)
    };

    draw_text_clipped(fb, pitch, scr_w, scr_h, tx, ty, x + w - 2, label, text_col, None);

    is_hover && clicked
}

// =============================================================================
// 3. IMMEDIATE-MODE TAB BAR
// =============================================================================

/// 3. Immediate-Mode Tab Bar with 4px top rounded corners, active white tab with
/// 2px luminous sapphire accent line, translucent idle tabs, and click tab switching.
/// Retorna `usize` da aba ativa.
pub unsafe fn ui_tab_bar(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    tabs: &[&str],
    active_tab: usize,
    mx: i32,
    my: i32,
    clicked: bool,
) -> usize {
    let bar_h = 28usize;

    // Background dock toolbar strip
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, x, y, w, bar_h, 0, COLOR_ICE_BLUE_TOP, COLOR_ICE_BLUE_BOT);
    // 1px bottom dividing line
    let div_y = y + bar_h - 1;
    if div_y < scr_h {
        let row_ptr = fb.add(div_y * pitch) as *mut u32;
        let x_end = (x + w).min(scr_w);
        for px in x..x_end {
            *row_ptr.add(px) = COLOR_BORDER_SUBTLE;
        }
    }

    let mut current_active = active_tab;
    let mut tab_x = x + 8;

    for (idx, tab_name) in tabs.iter().enumerate() {
        let tab_text_w = tab_name.chars().count() * 8;
        let tab_w = tab_text_w + 24;
        let tab_h = bar_h - 2;
        let is_active = current_active == idx;
        let is_hover = mx >= tab_x as i32
            && mx < (tab_x + tab_w) as i32
            && my >= (y + 2) as i32
            && my < (y + bar_h) as i32;

        let tab_y = y + 2;

        if is_active {
            // Active tab: Crisp white chassis with top rounded corners (4px)
            fill_top_rounded_rect(fb, pitch, scr_w, scr_h, tab_x, tab_y, tab_w, tab_h, 4, COLOR_ICE_WHITE);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, tab_x, tab_y, tab_w, tab_h, 4, COLOR_BORDER_SUBTLE);

            // 2px Sapphire highlight accent line along the bottom border of the tab
            let accent_y1 = y + bar_h - 2;
            let accent_y2 = y + bar_h - 1;
            if accent_y2 < scr_h {
                let row1 = fb.add(accent_y1 * pitch) as *mut u32;
                let row2 = fb.add(accent_y2 * pitch) as *mut u32;
                let x_end = (tab_x + tab_w).min(scr_w);
                for px in tab_x..x_end {
                    *row1.add(px) = COLOR_SAPPHIRE_PRIMARY;
                    *row2.add(px) = COLOR_SAPPHIRE_PRIMARY;
                }
            }

            // Top chamfer line
            if tab_y + 1 < scr_h && tab_w > 4 {
                let top_row = fb.add((tab_y + 1) * pitch) as *mut u32;
                let x_start = (tab_x + 2).min(scr_w);
                let x_end = (tab_x + tab_w.saturating_sub(2)).min(scr_w);
                for px in x_start..x_end {
                    let c = *top_row.add(px);
                    *top_row.add(px) = blend_argb(c, COLOR_ICE_WHITE, 140);
                }
            }

            let text_x = tab_x + (tab_w.saturating_sub(tab_text_w)) / 2;
            draw_text_clipped(fb, pitch, scr_w, scr_h, text_x, tab_y + 5, tab_x + tab_w - 4, tab_name, COLOR_SAPPHIRE_PRIMARY, None);
        } else {
            // Inactive tab: Translucent / subtle pastel chassis
            let bg_color = if is_hover { COLOR_ICE_HOVER_GLOW } else { COLOR_ICE_BLUE_BOT };
            let border_color = if is_hover { COLOR_SAPPHIRE_HOVER } else { COLOR_BORDER_SUBTLE };
            let text_color = if is_hover { COLOR_SAPPHIRE_PRIMARY } else { COLOR_TEXT_MUTED };

            fill_top_rounded_rect(fb, pitch, scr_w, scr_h, tab_x, tab_y + 2, tab_w, tab_h - 2, 4, bg_color);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, tab_x, tab_y + 2, tab_w, tab_h - 2, 4, border_color);

            let text_x = tab_x + (tab_w.saturating_sub(tab_text_w)) / 2;
            draw_text_clipped(fb, pitch, scr_w, scr_h, text_x, tab_y + 6, tab_x + tab_w - 4, tab_name, text_color, None);
        }

        if is_hover && clicked {
            current_active = idx;
        }

        tab_x += tab_w + 4;
    }

    current_active
}

// =============================================================================
// 4. IMMEDIATE-MODE TEXT INPUT
// =============================================================================

/// 4. Immediate-Mode Text Input Box with white background, translucent blue border,
/// horizontal text scroll, hardware PIT timer blinking vertical cursor, and PS/2 keyboard receiver.
/// Retorna `true` se o usuário clicou na caixa para focar.
pub unsafe fn ui_text_input(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    buffer: &mut String,
    is_focused: bool,
    mx: i32,
    my: i32,
    clicked: bool,
) -> bool {
    let is_hover = mx >= x as i32 && mx < (x + w) as i32 && my >= y as i32 && my < (y + h) as i32;
    let was_clicked = is_hover && clicked;

    // 1. Drain keyboard character buffer when focused
    if is_focused {
        while let Some(ch) = pop_keyboard_char() {
            match ch {
                '\x08' => {
                    // Backspace
                    buffer.pop();
                }
                '\n' | '\r' => {
                    // Enter (handled by callers or kept)
                }
                '\x1b' => {
                    // Escape
                }
                c if (c as u32) >= 32 && (c as u32) <= 126 => {
                    buffer.push(c);
                }
                _ => {}
            }
        }
    }

    // 2. Render white rectangular chassis with 3px rounded corners
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, w, h, 3, COLOR_ICE_WHITE);

    // 3. Render 1px top inner shadow bevel (recessed depth)
    if y + 1 < scr_h {
        let row_ptr = fb.add((y + 1) * pitch) as *mut u32;
        let x_start = (x + 2).min(scr_w);
        let x_end = (x + w.saturating_sub(2)).min(scr_w);
        for px in x_start..x_end {
            *row_ptr.add(px) = COLOR_ICE_LIGHT_GRAY;
        }
    }

    // 4. Border color: Translucent sapphire when focused/hover, subtle slate when idle
    let border = if is_focused {
        COLOR_SAPPHIRE_PRIMARY
    } else if is_hover {
        COLOR_SAPPHIRE_HOVER
    } else {
        COLOR_BORDER_SUBTLE
    };
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, w, h, 3, border);

    // 5. Horizontal text scroll calculation
    let inner_pad = 8usize;
    let avail_w = w.saturating_sub(inner_pad * 2);
    let max_vis_chars = (avail_w / 8).max(1);

    let char_count = buffer.chars().count();
    let (visible_str, cursor_char_pos) = if char_count > max_vis_chars {
        let skip = char_count - max_vis_chars;
        let byte_start = buffer
            .char_indices()
            .nth(skip)
            .map(|(idx, _)| idx)
            .unwrap_or(buffer.len());
        (&buffer[byte_start..], max_vis_chars)
    } else {
        (buffer.as_str(), char_count)
    };

    let text_x = x + inner_pad;
    let text_y = y + (h.saturating_sub(16)) / 2;
    draw_text_clipped(fb, pitch, scr_w, scr_h, text_x, text_y, x + w - inner_pad, visible_str, COLOR_TEXT_PRIMARY, None);

    // 6. Blinking vertical cursor based on hardware PIT ticks (~1.25 Hz)
    if is_focused {
        let ticks = SYSTEM_TICKS.load(Ordering::Relaxed);
        let cursor_visible = (ticks / 40) % 2 == 0;

        if cursor_visible {
            let cursor_x = text_x + cursor_char_pos * 8;
            if cursor_x + 2 < x + w && cursor_x < scr_w {
                let cy_start = y + 4;
                let cy_end = (y + h.saturating_sub(4)).min(scr_h);
                for py in cy_start..cy_end {
                    let row_ptr = fb.add(py * pitch) as *mut u32;
                    *row_ptr.add(cursor_x) = COLOR_SAPPHIRE_PRIMARY;
                    if cursor_x + 1 < scr_w {
                        *row_ptr.add(cursor_x + 1) = COLOR_SAPPHIRE_PRIMARY;
                    }
                }
            }
        }
    }

    was_clicked
}

// =============================================================================
// 5. IMMEDIATE-MODE TABLE VIEW
// =============================================================================

/// 5. Immediate-Mode Table View with Ice-Blue Header, alternating zebra striping
/// (#FFFFFF / #F8FAFC), row selection tracking, and interactive vitrified scrollbar.
/// Retorna `true` se alguma linha foi clicada.
pub unsafe fn ui_table_view(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    headers: &[(&str, usize)],
    rows: &[Vec<String>],
    selected_row: &mut usize,
    scroll_offset: &mut usize,
    mx: i32,
    my: i32,
    clicked: bool,
) -> bool {
    if w < 32 || h < 40 || fb.is_null() {
        return false;
    }

    let mut row_clicked = false;
    let sb_w = 16usize;
    let content_w = w.saturating_sub(sb_w);
    let header_h = 24usize;
    let row_h = 20usize;
    let max_vis_rows = (h.saturating_sub(header_h)) / row_h;

    // Background container
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, w, h, 2, COLOR_ICE_WHITE);

    // 1. Ice-Blue Table Header strip
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, x, y, content_w, header_h, 0, COLOR_ICE_BLUE_TOP, COLOR_ICE_BLUE_BOT);
    // 1px bottom border on header
    if y + header_h - 1 < scr_h {
        let row_ptr = fb.add((y + header_h - 1) * pitch) as *mut u32;
        for px in x..(x + content_w).min(scr_w) {
            *row_ptr.add(px) = COLOR_BORDER_SUBTLE;
        }
    }

    let mut h_x = x + 8;
    for (col_idx, (h_name, col_w)) in headers.iter().enumerate() {
        draw_text_clipped(fb, pitch, scr_w, scr_h, h_x, y + 4, h_x + col_w - 4, h_name, COLOR_TEXT_SECONDARY, None);
        // Column divider
        let div_x = h_x + col_w - 2;
        if col_idx + 1 < headers.len() && div_x < x + content_w && div_x < scr_w {
            for py in (y + 3)..(y + header_h - 3).min(scr_h) {
                let ptr = fb.add(py * pitch) as *mut u32;
                *ptr.add(div_x) = COLOR_BORDER_SUBTLE;
            }
        }
        h_x += col_w;
    }

    // 2. Table Rows with Alternating Zebra Striping
    let total_rows = rows.len();
    let max_scroll = total_rows.saturating_sub(max_vis_rows);
    if *scroll_offset > max_scroll {
        *scroll_offset = max_scroll;
    }

    for i in 0..max_vis_rows {
        let row_y = y + header_h + i * row_h;
        if row_y + row_h > y + h {
            break;
        }

        let data_idx = *scroll_offset + i;
        if data_idx >= total_rows {
            // Fill residual space with alternating stripes
            let empty_bg = if data_idx % 2 == 1 { COLOR_ICE_LIGHT_GRAY } else { COLOR_ICE_WHITE };
            fill_rounded_rect(fb, pitch, scr_w, scr_h, x, row_y, content_w, row_h, 0, empty_bg);
            continue;
        }

        let is_sel = *selected_row == data_idx;
        let is_row_hover = mx >= x as i32
            && mx < (x + content_w) as i32
            && my >= row_y as i32
            && my < (row_y + row_h) as i32;

        let bg_color = if is_sel {
            COLOR_ICE_SELECTED_BG
        } else if is_row_hover {
            COLOR_ICE_HOVER_GLOW
        } else if data_idx % 2 == 1 {
            COLOR_ICE_LIGHT_GRAY
        } else {
            COLOR_ICE_WHITE
        };

        fill_rounded_rect(fb, pitch, scr_w, scr_h, x, row_y, content_w, row_h, 0, bg_color);

        if is_sel {
            // Draw subtle selection highlight stroke on row bottom
            let row_bot = row_y + row_h - 1;
            if row_bot < scr_h {
                let row_ptr = fb.add(row_bot * pitch) as *mut u32;
                for px in x..(x + content_w).min(scr_w) {
                    *row_ptr.add(px) = COLOR_SAPPHIRE_HOVER;
                }
            }
        }

        // Render cells
        let row_data = &rows[data_idx];
        let mut cell_x = x + 8;
        for (c_idx, cell_str) in row_data.iter().enumerate() {
            let col_w = headers.get(c_idx).map(|h| h.1).unwrap_or(100);
            let text_color = if is_sel { COLOR_SAPPHIRE_DEEP } else { COLOR_TEXT_PRIMARY };
            draw_text_clipped(fb, pitch, scr_w, scr_h, cell_x, row_y + 2, cell_x + col_w - 4, cell_str, text_color, None);
            cell_x += col_w;
        }

        if is_row_hover && clicked {
            *selected_row = data_idx;
            row_clicked = true;
        }
    }

    // 3. Lateral Vitrified Scrollbar
    let sb_x = x + content_w;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, sb_x, y, sb_w, h, 0, COLOR_ICE_LIGHT_GRAY);
    // Vertical divider line separating table content and scrollbar
    if sb_x < scr_w {
        for py in y..(y + h).min(scr_h) {
            let ptr = fb.add(py * pitch) as *mut u32;
            *ptr.add(sb_x) = COLOR_BORDER_SUBTLE;
        }
    }

    // Scrollbar [ ▲ ] Button
    let up_hover = mx >= sb_x as i32 && mx < (sb_x + sb_w) as i32 && my >= y as i32 && my < (y + 16) as i32;
    let up_bg = if up_hover { COLOR_ICE_HOVER_GLOW } else { COLOR_ICE_BLUE_TOP };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, sb_x, y, sb_w, 16, 0, up_bg);
    draw_text_centered(fb, pitch, scr_w, scr_h, sb_x, y, sb_w, 16, "^", COLOR_TEXT_MUTED);
    if up_hover && clicked {
        *scroll_offset = scroll_offset.saturating_sub(1);
    }

    // Scrollbar [ ▼ ] Button
    let down_y = y + h - 16;
    let down_hover = mx >= sb_x as i32 && mx < (sb_x + sb_w) as i32 && my >= down_y as i32 && my < (y + h) as i32;
    let down_bg = if down_hover { COLOR_ICE_HOVER_GLOW } else { COLOR_ICE_BLUE_TOP };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, sb_x, down_y, sb_w, 16, 0, down_bg);
    draw_text_centered(fb, pitch, scr_w, scr_h, sb_x, down_y, sb_w, 16, "v", COLOR_TEXT_MUTED);
    if down_hover && clicked {
        if *scroll_offset < max_scroll {
            *scroll_offset += 1;
        }
    }

    // Vitrified Scrollbar Thumb
    let track_y = y + 16;
    let track_h = h.saturating_sub(32);
    if track_h > 12 && total_rows > max_vis_rows {
        let thumb_h = ((max_vis_rows * track_h) / total_rows).clamp(16, track_h);
        let travel = track_h.saturating_sub(thumb_h);
        let thumb_y = track_y + if max_scroll > 0 { (*scroll_offset * travel) / max_scroll } else { 0 };

        let thumb_hover = mx >= (sb_x + 2) as i32
            && mx < (sb_x + sb_w - 2) as i32
            && my >= thumb_y as i32
            && my < (thumb_y + thumb_h) as i32;

        let (thumb_top, thumb_bot) = if thumb_hover {
            (COLOR_ICE_WHITE, COLOR_ICE_SELECTED_BG)
        } else {
            (COLOR_ICE_WHITE, COLOR_BORDER_SUBTLE)
        };

        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, sb_x + 2, thumb_y, sb_w - 4, thumb_h, 3, thumb_top, thumb_bot);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sb_x + 2, thumb_y, sb_w - 4, thumb_h, 3, COLOR_BORDER_STRONG);

        // Click on track to jump
        let in_track = mx >= sb_x as i32 && mx < (sb_x + sb_w) as i32 && my >= track_y as i32 && my < (track_y + track_h) as i32;
        if in_track && clicked && !thumb_hover {
            let rel_y = (my - track_y as i32).max(0) as usize;
            *scroll_offset = if travel > 0 { (rel_y * max_scroll) / travel } else { 0 };
        }
    }

    // Outer table border
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, w, h, 2, COLOR_BORDER_SUBTLE);

    row_clicked
}

// =============================================================================
// 6. IMMEDIATE-MODE PROGRESS BAR
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgressBarTheme {
    SapphireBlue,
    EmeraldGreen,
    RubyCritical,
}

/// 6. Immediate-Mode Progress Bar with chamfered vitrified frame, liquid gradient fill
/// (Emerald Green or Sapphire Blue), and automatic Ruby Red alert when above 90% or critical.
pub unsafe fn ui_progress_bar(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    percent: usize,
    is_critical: bool,
) {
    ui_progress_bar_themed(fb, pitch, scr_w, scr_h, x, y, w, h, percent, is_critical, ProgressBarTheme::SapphireBlue);
}

/// Progress bar with explicit theme selection.
pub unsafe fn ui_progress_bar_themed(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    percent: usize,
    is_critical: bool,
    theme: ProgressBarTheme,
) {
    if w < 4 || h < 4 || fb.is_null() {
        return;
    }

    let clamped_pct = percent.min(100);

    // 1. Recessed vitrified chassis background
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, w, h, 4, COLOR_ICE_LIGHT_GRAY);

    // 1px top inner shadow bevel
    if y + 1 < scr_h {
        let row_ptr = fb.add((y + 1) * pitch) as *mut u32;
        let x_start = (x + 2).min(scr_w);
        let x_end = (x + w.saturating_sub(2)).min(scr_w);
        for px in x_start..x_end {
            *row_ptr.add(px) = COLOR_BORDER_SUBTLE;
        }
    }

    // Outer chamfer border
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, w, h, 4, COLOR_BORDER_STRONG);

    // 2. Determine liquid color gradient
    let (c_top, c_bot) = if is_critical || clamped_pct >= 90 {
        (COLOR_RUBY_ALERT_TOP, COLOR_RUBY_ALERT_BOT)
    } else {
        match theme {
            ProgressBarTheme::EmeraldGreen => (COLOR_EMERALD_TOP, COLOR_EMERALD_BOT),
            _ => (COLOR_SAPPHIRE_HOVER, COLOR_SAPPHIRE_PRIMARY),
        }
    };

    let fill_w = (w.saturating_sub(2) * clamped_pct) / 100;
    if fill_w > 0 {
        let fill_h = h.saturating_sub(2);
        fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, x + 1, y + 1, fill_w, fill_h, 3, c_top, c_bot);

        // 3. Liquid specular reflection sheen along upper 40%
        let sheen_h = (fill_h * 40) / 100;
        for sy in 0..sheen_h {
            let py = y + 1 + sy;
            if py >= scr_h {
                break;
            }
            let alpha = ((sheen_h - sy) * 50 / sheen_h).clamp(0, 255) as u8;
            let row_ptr = fb.add(py * pitch) as *mut u32;
            let x_start = (x + 2).min(scr_w);
            let x_end = (x + 1 + fill_w).min(scr_w);
            for px in x_start..x_end {
                let c = *row_ptr.add(px);
                *row_ptr.add(px) = blend_argb(c, COLOR_ICE_WHITE, alpha);
            }
        }

        // 1px bright liquid meniscus line along the top edge
        let top_y = y + 1;
        if top_y < scr_h {
            let row_ptr = fb.add(top_y * pitch) as *mut u32;
            let x_start = (x + 2).min(scr_w);
            let x_end = (x + fill_w).min(scr_w);
            for px in x_start..x_end {
                let c = *row_ptr.add(px);
                *row_ptr.add(px) = blend_argb(c, COLOR_ICE_WHITE, 120);
            }
        }
    }
}

// =============================================================================
// 7. IMMEDIATE-MODE SLIDER
// =============================================================================

/// 7. Immediate-Mode Horizontal Slider with 4px recessed track, 14px circular
/// vitrified knob, proportional mouse drag calculation, and min..max clamping.
/// Retorna `true` se o valor foi alterado.
pub unsafe fn ui_slider(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    value: &mut u8,
    min: u8,
    max: u8,
    mx: i32,
    my: i32,
    mouse_down: bool,
) -> bool {
    if w < 20 || fb.is_null() {
        return false;
    }

    let knob_d = 14usize;
    let knob_r = 7usize;
    let track_h = 4usize;
    let track_y = y + (knob_d - track_h) / 2;

    let range = (max.saturating_sub(min)).max(1) as usize;
    let cur = (*value as usize).saturating_sub(min as usize);
    let travel = w.saturating_sub(knob_d);
    let knob_x = x + (cur * travel) / range;
    let knob_y = y;

    // 1. Draw recessed 4px track background
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, track_y, w, track_h, 2, COLOR_BORDER_SUBTLE);
    // Inner track shadow line
    if track_y < scr_h {
        let row_ptr = fb.add(track_y * pitch) as *mut u32;
        for px in x..(x + w).min(scr_w) {
            *row_ptr.add(px) = COLOR_BORDER_STRONG;
        }
    }

    // 2. Active track fill (Sapphire Blue)
    let fill_w = (knob_x + knob_r).saturating_sub(x).min(w);
    if fill_w > 0 {
        fill_rounded_rect(fb, pitch, scr_w, scr_h, x, track_y, fill_w, track_h, 2, COLOR_SAPPHIRE_PRIMARY);
    }

    // 3. Vitrified circular knob (14x14px)
    let is_hover = mx >= knob_x as i32
        && mx < (knob_x + knob_d) as i32
        && my >= knob_y as i32
        && my < (knob_y + knob_d) as i32;

    let (knob_top, knob_bot) = if is_hover {
        (COLOR_ICE_WHITE, COLOR_ICE_SELECTED_BG)
    } else {
        (COLOR_ICE_WHITE, COLOR_ICE_BLUE_BOT)
    };

    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, knob_x, knob_y, knob_d, knob_d, knob_r, knob_top, knob_bot);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, knob_x, knob_y, knob_d, knob_d, knob_r, COLOR_SAPPHIRE_PRIMARY);

    // Specular light point on knob
    let dot_x = knob_x + 4;
    let dot_y = knob_y + 3;
    if dot_x < scr_w && dot_y < scr_h {
        let ptr = fb.add(dot_y * pitch) as *mut u32;
        *ptr.add(dot_x) = COLOR_ICE_WHITE;
        if dot_x + 1 < scr_w {
            *ptr.add(dot_x + 1) = COLOR_ICE_WHITE;
        }
    }

    // 4. Hitbox & Mouse Drag Tracking
    let in_bounds = mx >= (x as i32 - 4)
        && mx <= ((x + w) as i32 + 4)
        && my >= (y as i32 - 4)
        && my <= ((y + knob_d) as i32 + 4);

    if mouse_down && in_bounds {
        let rel_x = (mx - x as i32 - knob_r as i32).clamp(0, travel as i32) as usize;
        let new_val = (min as usize + (rel_x * range + travel / 2) / travel).clamp(min as usize, max as usize) as u8;
        if *value != new_val {
            *value = new_val;
            return true;
        }
    }

    false
}

/// Slider variant supporting floating-point ranges [min..max].
pub unsafe fn ui_slider_f32(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    value: &mut f32,
    min: f32,
    max: f32,
    mx: i32,
    my: i32,
    mouse_down: bool,
) -> bool {
    let mut int_val = (((*value - min) / (max - min).max(0.0001)) * 100.0).clamp(0.0, 100.0) as u8;
    let changed = ui_slider(fb, pitch, scr_w, scr_h, x, y, w, &mut int_val, 0, 100, mx, my, mouse_down);
    if changed {
        *value = min + (int_val as f32 / 100.0) * (max - min);
    }
    changed
}

/// Vertical Slider variant with 4px recessed vertical track and 14px vitrified knob.
pub unsafe fn ui_slider_vertical(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    h: usize,
    value: &mut u8,
    min: u8,
    max: u8,
    mx: i32,
    my: i32,
    mouse_down: bool,
) -> bool {
    if h < 20 || fb.is_null() {
        return false;
    }

    let knob_d = 14usize;
    let knob_r = 7usize;
    let track_w = 4usize;
    let track_x = x + (knob_d - track_w) / 2;

    let range = (max.saturating_sub(min)).max(1) as usize;
    let cur = (*value as usize).saturating_sub(min as usize);
    let travel = h.saturating_sub(knob_d);
    // Inverted vertical axis: bottom is min, top is max
    let knob_y = y + travel.saturating_sub((cur * travel) / range);
    let knob_x = x;

    // 1. Recessed vertical track
    fill_rounded_rect(fb, pitch, scr_w, scr_h, track_x, y, track_w, h, 2, COLOR_BORDER_SUBTLE);

    // 2. Active track fill (from bottom to knob)
    let fill_y = knob_y + knob_r;
    let fill_h = (y + h).saturating_sub(fill_y);
    if fill_h > 0 {
        fill_rounded_rect(fb, pitch, scr_w, scr_h, track_x, fill_y, track_w, fill_h, 2, COLOR_SAPPHIRE_PRIMARY);
    }

    // 3. Vitrified knob
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, knob_x, knob_y, knob_d, knob_d, knob_r, COLOR_ICE_WHITE, COLOR_ICE_BLUE_BOT);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, knob_x, knob_y, knob_d, knob_d, knob_r, COLOR_SAPPHIRE_PRIMARY);

    // 4. Drag detection
    let in_bounds = mx >= (x as i32 - 4)
        && mx <= ((x + knob_d) as i32 + 4)
        && my >= (y as i32 - 4)
        && my <= ((y + h) as i32 + 4);

    if mouse_down && in_bounds {
        let rel_y = (my - y as i32 - knob_r as i32).clamp(0, travel as i32) as usize;
        let inv_rel = travel.saturating_sub(rel_y);
        let new_val = (min as usize + (inv_rel * range + travel / 2) / travel).clamp(min as usize, max as usize) as u8;
        if *value != new_val {
            *value = new_val;
            return true;
        }
    }

    false
}

// =============================================================================
// 8. IMMEDIATE-MODE CHECKBOX & RADIO BUTTON
// =============================================================================

/// 8. Immediate-Mode Checkbox (14x14px) with rounded corners (3px), white background,
/// blue border, sapphire-blue checkmark 'v' when checked, and click toggle.
/// Retorna `true` se o estado foi alterado pelo clique.
pub unsafe fn ui_checkbox(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    label: &str,
    checked: &mut bool,
    mx: i32,
    my: i32,
    clicked: bool,
) -> bool {
    let box_size = 14usize;
    let total_w = box_size + 8 + label.chars().count() * 8;
    let is_hover = mx >= x as i32 && mx < (x + total_w) as i32 && my >= y as i32 && my < (y + 16) as i32;

    // 1. Box chassis: White background with 3px rounded corners
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, box_size, box_size, 3, COLOR_ICE_WHITE);

    // 2. Border: Sapphire highlight on hover, Slate subtle when idle
    let border_color = if *checked {
        COLOR_SAPPHIRE_PRIMARY
    } else if is_hover {
        COLOR_SAPPHIRE_HOVER
    } else {
        COLOR_BORDER_STRONG
    };
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, box_size, box_size, 3, border_color);

    // 3. Sapphire Checkmark 'v' (2px thick vector stroke)
    if *checked {
        let check_col = COLOR_SAPPHIRE_PRIMARY;
        let stroke_pts = [
            (x + 3, y + 7), (x + 3, y + 8),
            (x + 4, y + 8), (x + 4, y + 9),
            (x + 5, y + 9), (x + 5, y + 10),
            (x + 6, y + 8), (x + 6, y + 9),
            (x + 7, y + 6), (x + 7, y + 7),
            (x + 8, y + 4), (x + 8, y + 5),
            (x + 9, y + 3), (x + 9, y + 4),
            (x + 10, y + 2), (x + 10, y + 3),
        ];

        for &(px, py) in &stroke_pts {
            if px < scr_w && py < scr_h {
                let ptr = fb.add(py * pitch) as *mut u32;
                *ptr.add(px) = check_col;
            }
        }
    }

    // 4. Label text
    let text_x = x + box_size + 6;
    let text_col = if is_hover { COLOR_SAPPHIRE_PRIMARY } else { COLOR_TEXT_PRIMARY };
    draw_text_clipped(fb, pitch, scr_w, scr_h, text_x, y, scr_w, label, text_col, None);

    if is_hover && clicked {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// Immediate-Mode Circular Radio Button (14x14px).
/// Retorna `true` se foi selecionado pelo clique.
pub unsafe fn ui_radio_button(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    label: &str,
    selected: bool,
    mx: i32,
    my: i32,
    clicked: bool,
) -> bool {
    let d = 14usize;
    let r = 7usize;
    let total_w = d + 8 + label.chars().count() * 8;
    let is_hover = mx >= x as i32 && mx < (x + total_w) as i32 && my >= y as i32 && my < (y + 16) as i32;

    // Circular white background
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, d, d, r, COLOR_ICE_WHITE);
    let border = if selected {
        COLOR_SAPPHIRE_PRIMARY
    } else if is_hover {
        COLOR_SAPPHIRE_HOVER
    } else {
        COLOR_BORDER_STRONG
    };
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, d, d, r, border);

    // Inner sapphire bullseye
    if selected {
        fill_rounded_rect(fb, pitch, scr_w, scr_h, x + 3, y + 3, 8, 8, 4, COLOR_SAPPHIRE_PRIMARY);
    }

    let text_x = x + d + 6;
    let text_col = if is_hover { COLOR_SAPPHIRE_PRIMARY } else { COLOR_TEXT_PRIMARY };
    draw_text_clipped(fb, pitch, scr_w, scr_h, text_x, y, scr_w, label, text_col, None);

    is_hover && clicked
}

// =============================================================================
// 9. IMMEDIATE-MODE BREADCRUMBS PATH BAR
// =============================================================================

/// 9. Immediate-Mode Breadcrumbs Path Bar with navigable clickable path segments
/// separated by '>', highlighting the hovered segment and returning the clicked segment index.
/// Retorna `Some(segment_index)` se algum segmento foi clicado, permitindo navegação no VFS.
pub unsafe fn ui_breadcrumbs_bar(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    path: &str,
    mx: i32,
    my: i32,
    clicked: bool,
) -> Option<usize> {
    if w < 32 || h < 16 || fb.is_null() {
        return None;
    }

    // 1. Vitrified container chassis
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, w, h, 3, COLOR_ICE_WHITE);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, x, y, w, h, 3, COLOR_BORDER_SUBTLE);

    // 2. Parse path segments
    let mut segments: Vec<&str> = Vec::new();
    for part in path.split(|c| c == '/' || c == '\\') {
        if !part.is_empty() {
            segments.push(part);
        }
    }
    if segments.is_empty() {
        segments.push("/");
    }

    let mut cur_x = x + 6;
    let mut clicked_segment: Option<usize> = None;
    let max_x = x + w.saturating_sub(6);

    for (idx, seg_name) in segments.iter().enumerate() {
        let seg_text_w = seg_name.chars().count() * 8;
        let seg_chip_w = seg_text_w + 12;

        if cur_x + seg_chip_w > max_x {
            // Path exceeds visible width, draw ellipsis
            draw_text_clipped(fb, pitch, scr_w, scr_h, cur_x, y + (h.saturating_sub(16)) / 2, max_x, "...", COLOR_TEXT_MUTED, None);
            break;
        }

        let chip_y = y + 2;
        let chip_h = h.saturating_sub(4);
        let is_hover = mx >= cur_x as i32
            && mx < (cur_x + seg_chip_w) as i32
            && my >= chip_y as i32
            && my < (chip_y + chip_h) as i32;

        if is_hover {
            fill_rounded_rect(fb, pitch, scr_w, scr_h, cur_x, chip_y, seg_chip_w, chip_h, 3, COLOR_ICE_HOVER_GLOW);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cur_x, chip_y, seg_chip_w, chip_h, 3, COLOR_SAPPHIRE_HOVER);
        }

        let text_col = if is_hover { COLOR_SAPPHIRE_PRIMARY } else { COLOR_TEXT_PRIMARY };
        let tx = cur_x + 6;
        let ty = y + (h.saturating_sub(16)) / 2;
        draw_text_clipped(fb, pitch, scr_w, scr_h, tx, ty, cur_x + seg_chip_w, seg_name, text_col, None);

        if is_hover && clicked {
            clicked_segment = Some(idx);
        }

        cur_x += seg_chip_w + 2;

        // Draw chevron separator '>' between segments
        if idx + 1 < segments.len() && cur_x + 10 < max_x {
            let sep_tx = cur_x + 2;
            draw_text_clipped(fb, pitch, scr_w, scr_h, sep_tx, ty, max_x, ">", COLOR_TEXT_MUTED, None);
            cur_x += 12;
        }
    }

    clicked_segment
}

/// Truncates a VFS path string in-place to the given segment index (0-based).
pub fn truncate_path_to_segment(path: &mut String, segment_index: usize) {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(|c| c == '/' || c == '\\') {
        if !part.is_empty() {
            parts.push(part);
        }
    }
    if segment_index < parts.len() {
        let is_dos = path.contains(':');
        let mut new_path = String::new();
        if !is_dos {
            new_path.push('/');
        }
        for (i, p) in parts.iter().take(segment_index + 1).enumerate() {
            if i > 0 || !is_dos {
                if !new_path.ends_with('/') && !new_path.ends_with('\\') {
                    new_path.push('/');
                }
            }
            new_path.push_str(p);
        }
        *path = new_path;
    }
}

// =============================================================================
// COMPOSITION PANELS — TOOLBARS & STATUS BARS
// =============================================================================

/// Renders a top Ice-Blue Gradient Glass Toolbar (#EBF3FC -> #D9E7F8) with subtle bottom divider.
pub unsafe fn ui_toolbar(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) {
    fill_rounded_rect_gradient(fb, pitch, scr_w, scr_h, x, y, w, h, 0, COLOR_ICE_BLUE_TOP, COLOR_ICE_BLUE_BOT);
    // 1px bottom dividing stroke
    let div_y = y + h - 1;
    if div_y < scr_h {
        let row_ptr = fb.add(div_y * pitch) as *mut u32;
        let x_end = (x + w).min(scr_w);
        for px in x..x_end {
            *row_ptr.add(px) = COLOR_BORDER_SUBTLE;
        }
    }
}

/// Renders a 22px Pearl-Gray Status Bar (#F1F5F9) along the bottom with left/right text indicators.
pub unsafe fn ui_statusbar(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    left_text: &str,
    right_text: &str,
) {
    fill_rounded_rect(fb, pitch, scr_w, scr_h, x, y, w, h, 0, COLOR_ICE_LIGHT_GRAY);
    // 1px top border line
    if y < scr_h {
        let row_ptr = fb.add(y * pitch) as *mut u32;
        let x_end = (x + w).min(scr_w);
        for px in x..x_end {
            *row_ptr.add(px) = COLOR_BORDER_SUBTLE;
        }
    }
    // Left status text
    draw_text_clipped(fb, pitch, scr_w, scr_h, x + 8, y + 3, x + w / 2, left_text, COLOR_TEXT_SECONDARY, None);

    // Vertical divider at mid
    let div_x = x + w / 2;
    for py in (y + 3)..(y + h.saturating_sub(3)).min(scr_h) {
        if div_x < scr_w {
            let row_ptr = fb.add(py * pitch) as *mut u32;
            *row_ptr.add(div_x) = COLOR_BORDER_SUBTLE;
        }
    }
    // Right status text
    if !right_text.is_empty() {
        draw_text_clipped(fb, pitch, scr_w, scr_h, div_x + 8, y + 3, x + w - 8, right_text, COLOR_TEXT_MUTED, None);
    }
}
