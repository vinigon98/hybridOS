//! hyb-chars: Embedded Unicode Inspector & Technical Symbols Catalog
//!
//! Production-grade Unicode browser with deep character encoding analysis
//! (UTF-8 bytes, UTF-16 code units, HTML entities, Rust escapes) and instant search.

use libhyb_cfg::ConfigStore;
use libhyb_ui::colors::*;
use libhyb_ui::surface::Surface;
use libhyb_ui::widgets::{Button, TabBar, TextInput};
use std::env;
use std::path::Path;

/// Unicode Character Record
#[derive(Debug, Clone)]
pub struct UnicodeEntry {
    pub codepoint: u32,
    pub character: char,
    pub name: &'static str,
    pub category: &'static str,
    pub html_entity: &'static str,
}

impl UnicodeEntry {
    pub const fn new(cp: u32, ch: char, name: &'static str, cat: &'static str, html: &'static str) -> Self {
        Self {
            codepoint: cp,
            character: ch,
            name,
            category: cat,
            html_entity: html,
        }
    }

    pub fn utf8_bytes(&self) -> Vec<u8> {
        let mut b = [0u8; 4];
        let s = self.character.encode_utf8(&mut b);
        s.as_bytes().to_vec()
    }

    pub fn utf8_hex_str(&self) -> String {
        let bytes = self.utf8_bytes();
        bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
    }

    pub fn rust_escape(&self) -> String {
        format!("\\u{{{:04X}}}", self.codepoint)
    }
}

/// Embedded Standard Technical & Mathematical Unicode Database
pub static UNICODE_DB: &[UnicodeEntry] = &[
    UnicodeEntry::new(0x2192, '→', "RIGHTWARDS ARROW", "Arrows", "&rarr;"),
    UnicodeEntry::new(0x2190, '←', "LEFTWARDS ARROW", "Arrows", "&larr;"),
    UnicodeEntry::new(0x2191, '↑', "UPWARDS ARROW", "Arrows", "&uarr;"),
    UnicodeEntry::new(0x2193, '↓', "DOWNWARDS ARROW", "Arrows", "&darr;"),
    UnicodeEntry::new(0x21D2, '⇒', "RIGHTWARDS DOUBLE ARROW", "Arrows", "&rArr;"),
    UnicodeEntry::new(0x221A, '√', "SQUARE ROOT", "Math", "&radic;"),
    UnicodeEntry::new(0x221E, '∞', "INFINITY", "Math", "&infin;"),
    UnicodeEntry::new(0x222B, '∫', "INTEGRAL", "Math", "&int;"),
    UnicodeEntry::new(0x2248, '≈', "ALMOST EQUAL TO", "Math", "&asymp;"),
    UnicodeEntry::new(0x2260, '≠', "NOT EQUAL TO", "Math", "&ne;"),
    UnicodeEntry::new(0x2264, '≤', "LESS-THAN OR EQUAL TO", "Math", "&le;"),
    UnicodeEntry::new(0x2265, '≥', "GREATER-THAN OR EQUAL TO", "Math", "&ge;"),
    UnicodeEntry::new(0x00B1, '±', "PLUS-MINUS SIGN", "Math", "&plusmn;"),
    UnicodeEntry::new(0x00D7, '×', "MULTIPLICATION SIGN", "Math", "&times;"),
    UnicodeEntry::new(0x00F7, '÷', "DIVISION SIGN", "Math", "&divide;"),
    UnicodeEntry::new(0x03C0, 'π', "GREEK SMALL LETTER PI", "Greek", "&pi;"),
    UnicodeEntry::new(0x03A3, 'Σ', "GREEK CAPITAL LETTER SIGMA", "Greek", "&Sigma;"),
    UnicodeEntry::new(0x03A9, 'Ω', "GREEK CAPITAL LETTER OMEGA", "Greek", "&Omega;"),
    UnicodeEntry::new(0x03BB, 'λ', "GREEK SMALL LETTER LAMBDA", "Greek", "&lambda;"),
    UnicodeEntry::new(0x2500, '─', "BOX DRAWINGS LIGHT HORIZONTAL", "Box", "&#9472;"),
    UnicodeEntry::new(0x2502, '│', "BOX DRAWINGS LIGHT VERTICAL", "Box", "&#9474;"),
    UnicodeEntry::new(0x250C, '┌', "BOX DRAWINGS LIGHT DOWN AND RIGHT", "Box", "&#9484;"),
    UnicodeEntry::new(0x2510, '┐', "BOX DRAWINGS LIGHT DOWN AND LEFT", "Box", "&#9488;"),
    UnicodeEntry::new(0x2514, '└', "BOX DRAWINGS LIGHT UP AND RIGHT", "Box", "&#9492;"),
    UnicodeEntry::new(0x2518, '┘', "BOX DRAWINGS LIGHT UP AND LEFT", "Box", "&#9496;"),
    UnicodeEntry::new(0x2318, '⌘', "PLACE OF INTEREST SIGN (COMMAND)", "Technical", "&#8984;"),
    UnicodeEntry::new(0x2325, '⌥', "OPTION KEY", "Technical", "&#8997;"),
    UnicodeEntry::new(0x21E7, '⇧', "UPWARDS WHITE ARROW (SHIFT)", "Technical", "&#8679;"),
];

/// Chars Application State
pub struct CharsAppState {
    pub tab_bar: TabBar,
    pub search_input: TextInput,
    pub filtered_entries: Vec<usize>,
    pub selected_index: usize,
    pub copy_btn: Button,
}

impl CharsAppState {
    pub fn new() -> Self {
        let mut tab_bar = TabBar::new(20, 20, 500, 32);
        tab_bar.add_tab("All");
        tab_bar.add_tab("Math");
        tab_bar.add_tab("Arrows");
        tab_bar.add_tab("Greek");
        tab_bar.add_tab("Technical");

        let search_input = TextInput::new(540, 20, 300, 32, "Search character, name, or U+XXXX...");
        let copy_btn = Button::new(850, 20, 100, 32, "Copy Glyph").with_accent();

        let filtered_entries = (0..UNICODE_DB.len()).collect();

        Self {
            tab_bar,
            search_input,
            filtered_entries,
            selected_index: 0,
            copy_btn,
        }
    }

    pub fn filter_by_query(&mut self, query: &str) {
        let q = query.trim().to_lowercase();
        self.filtered_entries.clear();

        for (idx, entry) in UNICODE_DB.iter().enumerate() {
            if q.is_empty()
                || entry.name.to_lowercase().contains(&q)
                || format!("u+{:04x}", entry.codepoint).contains(&q)
                || entry.html_entity.to_lowercase().contains(&q)
                || entry.character.to_string() == q
            {
                self.filtered_entries.push(idx);
            }
        }
        self.selected_index = 0;
    }

    pub fn render(&self, surface: &mut Surface) {
        surface.fill_rect(0, 0, surface.width as u32, surface.height as u32, COLOR_CARD);

        // Header
        self.tab_bar.render(surface);
        self.search_input.render(surface);
        self.copy_btn.render(surface);

        // Grid of Characters
        let grid_x = 20;
        let grid_y = 70;
        let cell_size = 48;
        let cols = 12;

        for (grid_pos, &db_idx) in self.filtered_entries.iter().enumerate() {
            let row = grid_pos / cols;
            let col = grid_pos % cols;
            let cx = grid_x + (col as i32 * cell_size);
            let cy = grid_y + (row as i32 * cell_size);

            if cy + cell_size > surface.height as i32 - 160 {
                break;
            }

            let entry = &UNICODE_DB[db_idx];
            let is_sel = grid_pos == self.selected_index;
            let bg_col = if is_sel { COLOR_SELECTION } else { COLOR_SURFACE };

            surface.fill_rect(cx, cy, cell_size as u32 - 4, cell_size as u32 - 4, bg_col);
            let border_col = if is_sel { COLOR_ACCENT } else { COLOR_BORDER };
            surface.draw_rect(cx, cy, cell_size as u32 - 4, cell_size as u32 - 4, border_col);

            let char_str = entry.character.to_string();
            surface.draw_string((cx + 16) as usize, (cy + 14) as usize, &char_str, COLOR_TEXT);
        }

        // Inspector Panel at Bottom
        if let Some(&db_idx) = self.filtered_entries.get(self.selected_index) {
            let entry = &UNICODE_DB[db_idx];
            let panel_y = surface.height as i32 - 140;

            surface.fill_rect(20, panel_y, surface.width as u32 - 40, 120, COLOR_SURFACE);
            surface.draw_rect(20, panel_y, surface.width as u32 - 40, 120, COLOR_BORDER);

            // Large glyph display
            surface.fill_rect(32, panel_y + 16, 80, 88, COLOR_CARD);
            surface.draw_rect(32, panel_y + 16, 80, 88, COLOR_ACCENT);
            surface.draw_string(64, (panel_y + 44) as usize, &entry.character.to_string(), COLOR_TEXT);

            // Metadata fields
            let info_x = 130;
            surface.draw_string(info_x, (panel_y + 20) as usize, &format!("NAME:     {}", entry.name), COLOR_TEXT);
            surface.draw_string(
                info_x,
                (panel_y + 40) as usize,
                &format!("CODEPOINT: U+{:04X} ({}) | CATEGORY: {}", entry.codepoint, entry.codepoint, entry.category),
                COLOR_CYAN,
            );
            surface.draw_string(
                info_x,
                (panel_y + 60) as usize,
                &format!("UTF-8:    0x{} | ESCAPE: {}", entry.utf8_hex_str(), entry.rust_escape()),
                COLOR_SUCCESS,
            );
            surface.draw_string(
                info_x,
                (panel_y + 80) as usize,
                &format!("HTML:     {}", entry.html_entity),
                COLOR_TEXT_MUTED,
            );
        }
    }
}

fn print_usage() {
    println!("hyb-chars: HybridOS Embedded Unicode Inspector");
    println!("Usage: hyb-chars [OPTIONS]");
    println!("Options:");
    println!("  --screenshot <PATH>  Render UI to a PNG screenshot and exit");
    println!("  --search <QUERY>     Search and inspect character in terminal");
    println!("  --version            Display version information");
    println!("  --help               Display this help text");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let _config = ConfigStore::new();

    if args.contains(&"--version".to_string()) {
        println!("hyb-chars 0.1.0 (HybridOS Unicode Standard Engine)");
        return;
    }

    if args.contains(&"--help".to_string()) {
        print_usage();
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--search") {
        if let Some(q) = args.get(pos + 1) {
            println!("Searching Unicode database for '{}'...", q);
            for entry in UNICODE_DB {
                if entry.name.to_lowercase().contains(&q.to_lowercase())
                    || format!("u+{:04x}", entry.codepoint).contains(&q.to_lowercase())
                {
                    println!(
                        "  [{}] U+{:04X} - {} (UTF-8: {})",
                        entry.character,
                        entry.codepoint,
                        entry.name,
                        entry.utf8_hex_str()
                    );
                }
            }
            return;
        }
    }

    let mut state = CharsAppState::new();
    state.filter_by_query("arrow");

    println!("[hyb-chars] Unicode inspector online with {} total characters.", UNICODE_DB.len());

    let mut surface = Surface::new(980, 560, COLOR_BG);
    state.render(&mut surface);

    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        if let Some(path_str) = args.get(pos + 1) {
            let p = Path::new(path_str);
            match surface.export_png(p) {
                Ok(_) => println!("[hyb-chars] Screenshot written successfully: {}", path_str),
                Err(e) => eprintln!("[hyb-chars] Error exporting screenshot: {}", e),
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_utf8_encoding() {
        let arrow = &UNICODE_DB[0]; // → U+2192
        assert_eq!(arrow.codepoint, 0x2192);
        assert_eq!(arrow.utf8_bytes(), vec![0xE2, 0x86, 0x92]);
        assert_eq!(arrow.rust_escape(), "\\u{2192}");
    }

    #[test]
    fn test_search_filtering() {
        let mut state = CharsAppState::new();
        state.filter_by_query("integral");
        assert_eq!(state.filtered_entries.len(), 1);
        assert_eq!(UNICODE_DB[state.filtered_entries[0]].character, '∫');
    }
}
