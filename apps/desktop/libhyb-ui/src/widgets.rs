//! High-Performance Desktop UI Widgets for HybridOS

use crate::font::FONT_HEIGHT;
use crate::geometry::colors::*;
use crate::geometry::{Point, Rect};
use crate::icons::{draw_icon, IconType};
use crate::surface::Surface;
use std::collections::HashSet;

/// Interactive Push Button
#[derive(Debug, Clone)]
pub struct Button {
    pub rect: Rect,
    pub text: String,
    pub icon: Option<IconType>,
    pub is_hovered: bool,
    pub is_pressed: bool,
    pub is_accent: bool,
    pub is_danger: bool,
}

impl Button {
    pub fn new(x: i32, y: i32, width: u32, height: u32, text: &str) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
            text: text.to_string(),
            icon: None,
            is_hovered: false,
            is_pressed: false,
            is_accent: false,
            is_danger: false,
        }
    }

    pub fn with_icon(mut self, icon: IconType) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_accent(mut self) -> Self {
        self.is_accent = true;
        self
    }

    pub fn with_danger(mut self) -> Self {
        self.is_danger = true;
        self
    }

    pub fn handle_mouse(&mut self, p: Point, pressed: bool) -> bool {
        let inside = self.rect.contains(p);
        self.is_hovered = inside;
        let was_pressed = self.is_pressed;
        self.is_pressed = inside && pressed;
        inside && was_pressed && !pressed
    }

    pub fn render(&self, surface: &mut Surface) {
        let bg_color = if self.is_danger {
            if self.is_pressed {
                0xFFB00010
            } else if self.is_hovered {
                0xFFD01020
            } else {
                COLOR_DANGER
            }
        } else if self.is_accent {
            if self.is_pressed {
                0xFF005A9E
            } else if self.is_hovered {
                0xFF0086E8
            } else {
                COLOR_ACCENT
            }
        } else if self.is_pressed {
            0xFF383838
        } else if self.is_hovered {
            0xFF303030
        } else {
            COLOR_CARD
        };

        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, bg_color);
        let border_color = if self.is_hovered {
            COLOR_BORDER_ACTIVE
        } else {
            COLOR_BORDER
        };
        surface.draw_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, border_color);

        let mut text_x = (self.rect.x + 8) as usize;
        let text_y = (self.rect.y + ((self.rect.height as i32 - FONT_HEIGHT as i32) / 2).max(0)) as usize;

        if let Some(icon) = self.icon {
            draw_icon(surface, text_x, text_y, icon, COLOR_TEXT);
            text_x += 20;
        }

        surface.draw_string(text_x, text_y, &self.text, COLOR_TEXT);
    }
}

/// Editable Single-Line Text Input
#[derive(Debug, Clone)]
pub struct TextInput {
    pub rect: Rect,
    pub text: String,
    pub cursor_pos: usize,
    pub is_focused: bool,
    pub hint_text: String,
}

impl TextInput {
    pub fn new(x: i32, y: i32, width: u32, height: u32, hint_text: &str) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
            text: String::new(),
            cursor_pos: 0,
            is_focused: false,
            hint_text: hint_text.to_string(),
        }
    }

    pub fn handle_mouse(&mut self, p: Point) {
        self.is_focused = self.rect.contains(p);
    }

    pub fn handle_char(&mut self, ch: char) {
        if !self.is_focused {
            return;
        }
        if ch == '\x08' {
            if self.cursor_pos > 0 && !self.text.is_empty() {
                self.text.remove(self.cursor_pos - 1);
                self.cursor_pos -= 1;
            }
        } else if ch >= ' ' && ch <= '~' {
            self.text.insert(self.cursor_pos, ch);
            self.cursor_pos += 1;
        }
    }

    pub fn render(&self, surface: &mut Surface) {
        let bg_color = if self.is_focused { 0xFF202020 } else { 0xFF181818 };
        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, bg_color);
        let border_color = if self.is_focused { COLOR_ACCENT } else { COLOR_BORDER };
        surface.draw_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, border_color);

        let text_x = (self.rect.x + 8) as usize;
        let text_y = (self.rect.y + 6) as usize;

        if self.text.is_empty() && !self.is_focused {
            surface.draw_string(text_x, text_y, &self.hint_text, COLOR_TEXT_MUTED);
        } else {
            surface.draw_string(text_x, text_y, &self.text, COLOR_TEXT);
            if self.is_focused {
                let cursor_x = text_x + self.cursor_pos * 8;
                surface.fill_rect(cursor_x as i32, text_y as i32, 2, FONT_HEIGHT as u32, COLOR_ACCENT);
            }
        }
    }
}

/// Multiple Tab Container
#[derive(Debug, Clone)]
pub struct TabBar {
    pub rect: Rect,
    pub tabs: Vec<String>,
    pub active_tab: usize,
    pub hovered_tab: Option<usize>,
}

impl TabBar {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
            tabs: Vec::new(),
            active_tab: 0,
            hovered_tab: None,
        }
    }

    pub fn add_tab(&mut self, title: &str) {
        self.tabs.push(title.to_string());
    }

    pub fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    pub fn handle_mouse(&mut self, p: Point, clicked: bool) -> Option<usize> {
        if !self.rect.contains(p) {
            self.hovered_tab = None;
            return None;
        }

        let tab_w = 140;
        let clicked_idx = ((p.x - self.rect.x) / tab_w) as usize;
        if clicked_idx < self.tabs.len() {
            self.hovered_tab = Some(clicked_idx);
            if clicked {
                self.active_tab = clicked_idx;
                return Some(clicked_idx);
            }
        } else {
            self.hovered_tab = None;
        }
        None
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, COLOR_SURFACE);
        surface.fill_rect(self.rect.x, self.rect.bottom() - 1, self.rect.width, 1, COLOR_BORDER);

        let tab_w = 140;
        for (i, title) in self.tabs.iter().enumerate() {
            let tx = self.rect.x + (i as i32 * tab_w);
            let is_active = i == self.active_tab;
            let bg_color = if is_active { COLOR_CARD } else { COLOR_SURFACE };

            surface.fill_rect(tx, self.rect.y + 2, tab_w as u32 - 2, self.rect.height - 2, bg_color);
            if is_active {
                surface.fill_rect(tx, self.rect.y, tab_w as u32 - 2, 2, COLOR_ACCENT);
            }

            let text_color = if is_active { COLOR_TEXT } else { COLOR_TEXT_MUTED };
            let max_chars = 12;
            let display_text = if title.len() > max_chars {
                &title[..max_chars]
            } else {
                title.as_str()
            };
            surface.draw_string((tx + 12) as usize, (self.rect.y + 8) as usize, display_text, text_color);
            // Close tab marker
            surface.draw_string((tx + tab_w - 20) as usize, (self.rect.y + 8) as usize, "x", COLOR_TEXT_MUTED);
        }
    }
}

/// Sortable & Resizable Table View
#[derive(Debug, Clone)]
pub struct TableView {
    pub rect: Rect,
    pub columns: Vec<(String, u32)>, // (Header title, width)
    pub rows: Vec<Vec<String>>,
    pub selected_row: Option<usize>,
    pub sort_column: usize,
    pub sort_ascending: bool,
}

impl TableView {
    pub fn new(x: i32, y: i32, width: u32, height: u32, columns: Vec<(&str, u32)>) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
            columns: columns.into_iter().map(|(h, w)| (h.to_string(), w)).collect(),
            rows: Vec::new(),
            selected_row: None,
            sort_column: 0,
            sort_ascending: true,
        }
    }

    pub fn add_row(&mut self, row: Vec<&str>) {
        self.rows.push(row.into_iter().map(|s| s.to_string()).collect());
    }

    pub fn handle_click(&mut self, p: Point) {
        if !self.rect.contains(p) {
            return;
        }

        // Header click: sort
        let header_h = 24;
        if p.y >= self.rect.y && p.y < self.rect.y + header_h {
            let mut curr_x = self.rect.x;
            for (col_idx, (_, w)) in self.columns.iter().enumerate() {
                if p.x >= curr_x && p.x < curr_x + (*w as i32) {
                    if self.sort_column == col_idx {
                        self.sort_ascending = !self.sort_ascending;
                    } else {
                        self.sort_column = col_idx;
                        self.sort_ascending = true;
                    }
                    self.sort_rows();
                    break;
                }
                curr_x += *w as i32;
            }
            return;
        }

        // Row click
        let row_h = 22;
        let row_y_start = self.rect.y + header_h;
        if p.y >= row_y_start {
            let clicked_row = ((p.y - row_y_start) / row_h) as usize;
            if clicked_row < self.rows.len() {
                self.selected_row = Some(clicked_row);
            }
        }
    }

    fn sort_rows(&mut self) {
        let col = self.sort_column;
        let asc = self.sort_ascending;
        self.rows.sort_by(|a, b| {
            let val_a = a.get(col).map(|s| s.as_str()).unwrap_or("");
            let val_b = b.get(col).map(|s| s.as_str()).unwrap_or("");
            if asc {
                val_a.cmp(val_b)
            } else {
                val_b.cmp(val_a)
            }
        });
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, COLOR_CARD);
        surface.draw_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, COLOR_BORDER);

        // Header
        let header_h = 24;
        surface.fill_rect(self.rect.x + 1, self.rect.y + 1, self.rect.width - 2, header_h as u32, COLOR_TITLEBAR);
        surface.fill_rect(self.rect.x, self.rect.y + header_h, self.rect.width, 1, COLOR_BORDER);

        let mut curr_x = self.rect.x + 8;
        for (idx, (header, w)) in self.columns.iter().enumerate() {
            let arrow = if idx == self.sort_column {
                if self.sort_ascending { " ^" } else { " v" }
            } else {
                ""
            };
            let text = format!("{}{}", header, arrow);
            surface.draw_string(curr_x as usize, (self.rect.y + 5) as usize, &text, COLOR_TEXT_MUTED);
            curr_x += *w as i32;
        }

        // Rows
        let row_h = 22;
        for (r_idx, row) in self.rows.iter().enumerate() {
            let ry = self.rect.y + header_h + (r_idx as i32 * row_h);
            if ry + row_h > self.rect.bottom() {
                break;
            }

            let is_selected = self.selected_row == Some(r_idx);
            if is_selected {
                surface.fill_rect(self.rect.x + 1, ry, self.rect.width - 2, row_h as u32, COLOR_SELECTION);
            } else if r_idx % 2 == 1 {
                surface.fill_rect(self.rect.x + 1, ry, self.rect.width - 2, row_h as u32, 0x11FFFFFF);
            }

            let mut col_x = self.rect.x + 8;
            for (c_idx, cell) in row.iter().enumerate() {
                let cell_col = if is_selected { COLOR_TEXT } else { COLOR_TEXT_MUTED };
                surface.draw_string(col_x as usize, (ry + 4) as usize, cell, cell_col);
                if let Some((_, w)) = self.columns.get(c_idx) {
                    col_x += *w as i32;
                }
            }
        }
    }
}

/// Icon Grid with Multi-selection & Marquee Rubber-Band Box
#[derive(Debug, Clone)]
pub struct IconGrid {
    pub rect: Rect,
    pub items: Vec<(String, IconType, bool)>, // (Name, Icon, is_dir)
    pub selected_indices: HashSet<usize>,
    pub item_size: u32,
    pub rubber_band: Option<Rect>,
}

impl IconGrid {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            rect: Rect::new(x, y, width, height),
            items: Vec::new(),
            selected_indices: HashSet::new(),
            item_size: 72,
            rubber_band: None,
        }
    }

    pub fn add_item(&mut self, name: &str, icon: IconType, is_dir: bool) {
        self.items.push((name.to_string(), icon, is_dir));
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_indices.clear();
    }

    pub fn handle_click(&mut self, p: Point, ctrl_pressed: bool) {
        if !self.rect.contains(p) {
            return;
        }

        let cols = (self.rect.width / self.item_size).max(1);
        let rel_x = p.x - self.rect.x;
        let rel_y = p.y - self.rect.y;

        let col = (rel_x as u32) / self.item_size;
        let row = (rel_y as u32) / self.item_size;
        let index = (row * cols + col) as usize;

        if index < self.items.len() {
            if ctrl_pressed {
                if self.selected_indices.contains(&index) {
                    self.selected_indices.remove(&index);
                } else {
                    self.selected_indices.insert(index);
                }
            } else {
                self.selected_indices.clear();
                self.selected_indices.insert(index);
            }
        } else if !ctrl_pressed {
            self.selected_indices.clear();
        }
    }

    pub fn set_rubber_band(&mut self, start: Point, end: Point) {
        let box_rect = Rect::from_points(start, end);
        self.rubber_band = Some(box_rect);

        // Compute items intersected
        let cols = (self.rect.width / self.item_size).max(1);
        for (idx, _) in self.items.iter().enumerate() {
            let row = (idx as u32) / cols;
            let col = (idx as u32) % cols;
            let ix = self.rect.x + (col * self.item_size) as i32;
            let iy = self.rect.y + (row * self.item_size) as i32;
            let item_rect = Rect::new(ix, iy, self.item_size, self.item_size);
            if box_rect.intersects(&item_rect) {
                self.selected_indices.insert(idx);
            }
        }
    }

    pub fn clear_rubber_band(&mut self) {
        self.rubber_band = None;
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, COLOR_CARD);

        let cols = (self.rect.width / self.item_size).max(1);
        for (idx, (name, icon, _)) in self.items.iter().enumerate() {
            let row = (idx as u32) / cols;
            let col = (idx as u32) % cols;
            let ix = self.rect.x + (col * self.item_size) as i32;
            let iy = self.rect.y + (row * self.item_size) as i32;

            if iy + (self.item_size as i32) > self.rect.bottom() {
                break;
            }

            let is_sel = self.selected_indices.contains(&idx);
            if is_sel {
                surface.fill_rect(ix + 2, iy + 2, self.item_size - 4, self.item_size - 4, COLOR_SELECTION);
                surface.draw_rect(ix + 2, iy + 2, self.item_size - 4, self.item_size - 4, COLOR_ACCENT);
            }

            // Draw center icon
            let icon_x = (ix + (self.item_size as i32 - 16) / 2) as usize;
            let icon_y = (iy + 12) as usize;
            draw_icon(surface, icon_x, icon_y, *icon, COLOR_TEXT);

            // Draw text below
            let max_name_len = (self.item_size as usize / 8).saturating_sub(1);
            let display_name = if name.len() > max_name_len {
                &name[..max_name_len]
            } else {
                name.as_str()
            };
            let text_x = (ix + 6) as usize;
            let text_y = (iy + 36) as usize;
            surface.draw_string(text_x, text_y, display_name, COLOR_TEXT);
        }

        // Render Rubber-band marquee box if active
        if let Some(rb) = self.rubber_band {
            surface.fill_rect(rb.x, rb.y, rb.width, rb.height, 0x330078D4);
            surface.draw_rect(rb.x, rb.y, rb.width, rb.height, COLOR_ACCENT);
        }
    }
}

/// Floating Window Container Frame
#[derive(Debug, Clone)]
pub struct WindowFrame {
    pub rect: Rect,
    pub title: String,
    pub is_active: bool,
    pub close_button: Button,
    pub max_button: Button,
    pub min_button: Button,
}

impl WindowFrame {
    pub fn new(x: i32, y: i32, width: u32, height: u32, title: &str) -> Self {
        let btn_w = 36;
        let btn_h = 28;
        let right = x + width as i32;

        let close_btn = Button::new(right - btn_w - 2, y + 2, btn_w as u32, btn_h, "x").with_danger();
        let max_btn = Button::new(right - (btn_w * 2) - 4, y + 2, btn_w as u32, btn_h, "[]");
        let min_btn = Button::new(right - (btn_w * 3) - 6, y + 2, btn_w as u32, btn_h, "-");

        Self {
            rect: Rect::new(x, y, width, height),
            title: title.to_string(),
            is_active: true,
            close_button: close_btn,
            max_button: max_btn,
            min_button: min_btn,
        }
    }

    pub fn render(&self, surface: &mut Surface) {
        // Drop shadow
        surface.draw_drop_shadow(self.rect.x, self.rect.y, self.rect.width, self.rect.height, 6);

        // Window background
        surface.fill_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, COLOR_CARD);

        // Border
        let border_col = if self.is_active { COLOR_ACCENT } else { COLOR_BORDER };
        surface.draw_rect(self.rect.x, self.rect.y, self.rect.width, self.rect.height, border_col);

        // Titlebar
        let titlebar_h = 32;
        surface.fill_rect(self.rect.x + 1, self.rect.y + 1, self.rect.width - 2, titlebar_h, COLOR_TITLEBAR);
        surface.draw_string(
            (self.rect.x + 12) as usize,
            (self.rect.y + 8) as usize,
            &self.title,
            COLOR_TEXT,
        );

        // Control buttons
        self.min_button.render(surface);
        self.max_button.render(surface);
        self.close_button.render(surface);
    }
}
