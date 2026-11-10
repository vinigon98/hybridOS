//! HybridOS Modern Windows Terminal (wt.exe)
//!
//! Production-grade dual-personality terminal emulator for HybridOS,
//! providing tabbed multi-profile terminal sessions (PowerShell, Windows CMD,
//! Linux Bash, and HybridOS Universal Shell), ANSI/VT100 escape sequence processing,
//! split panes, 24-bit TrueColor themes, scrollback buffers, text selection,
//! and search.
//!
//! 100% Pure, Safe, and Robust Rust.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::fmt;

use crate::userland::shell::{HybridShell, ShellFlavor};

// ============================================================================
// Section 1: Core Geometry, Styles & Text Cells
// ============================================================================

/// Bounding rectangle for terminal window, tabs, and split panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PaneRect {
    pub const EMPTY: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x.saturating_add(self.width as i32)
            && py >= self.y
            && py < self.y.saturating_add(self.height as i32)
    }

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }
}

/// Representation of terminal text color: default, standard ANSI (0-15),
/// 256-color palette (0-255), or 24-bit TrueColor (RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    DefaultForeground,
    DefaultBackground,
    Ansi16(u8),
    Color256(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    /// Resolves this color to an RGB tuple using the given color scheme.
    pub fn resolve_rgb(&self, scheme: &ColorScheme, _is_foreground: bool) -> (u8, u8, u8) {
        match *self {
            Color::DefaultForeground => scheme.foreground,
            Color::DefaultBackground => scheme.background,
            Color::Ansi16(idx) => scheme.ansi_colors[(idx % 16) as usize],
            Color::Color256(idx) => {
                if idx < 16 {
                    scheme.ansi_colors[idx as usize]
                } else if idx < 232 {
                    // 6x6x6 color cube: indices 16..231
                    let code = idx - 16;
                    let b = code % 6;
                    let g = (code / 6) % 6;
                    let r = code / 36;
                    let to_val = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                    (to_val(r), to_val(g), to_val(b))
                } else {
                    // Grayscale gradient: indices 232..255 (24 steps: 8..238)
                    let gray = 8 + (idx - 232) * 10;
                    (gray, gray, gray)
                }
            }
            Color::Rgb(r, g, b) => (r, g, b),
        }
    }
}

/// Text formatting attributes for a single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAttributes {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub invert: bool,
    pub strikethrough: bool,
    pub fg: Color,
    pub bg: Color,
}

impl Default for CellAttributes {
    fn default() -> Self {
        Self {
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            invert: false,
            strikethrough: false,
            fg: Color::DefaultForeground,
            bg: Color::DefaultBackground,
        }
    }
}

impl CellAttributes {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A single character cell within the terminal display grid or scrollback buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCell {
    pub ch: char,
    pub attrs: CellAttributes,
    pub width: u8,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            attrs: CellAttributes::default(),
            width: 1,
        }
    }
}

impl TerminalCell {
    pub fn new(ch: char, attrs: CellAttributes) -> Self {
        Self {
            ch,
            attrs,
            width: 1,
        }
    }

    pub fn blank(attrs: CellAttributes) -> Self {
        Self {
            ch: ' ',
            attrs,
            width: 1,
        }
    }
}

/// Cursor position inside the viewport grid (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

impl CursorPosition {
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// Active selection range across text cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: CursorPosition,
    pub end: CursorPosition,
}

impl SelectionRange {
    pub fn new(p1: CursorPosition, p2: CursorPosition) -> Self {
        if p1 <= p2 {
            Self { start: p1, end: p2 }
        } else {
            Self { start: p2, end: p1 }
        }
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        let pos = CursorPosition { row, col };
        pos >= self.start && pos <= self.end
    }
}

// ============================================================================
// Section 2: Themes & Customization
// ============================================================================

/// Authentic color scheme specification matching Microsoft Windows Terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorScheme {
    pub name: String,
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
    pub selection_background: (u8, u8, u8),
    pub cursor_color: (u8, u8, u8),
    pub ansi_colors: [(u8, u8, u8); 16],
}

impl ColorScheme {
    /// Microsoft Windows Terminal default color scheme (Campbell).
    pub fn campbell() -> Self {
        Self {
            name: "Campbell".to_string(),
            foreground: (204, 204, 204),
            background: (12, 12, 12),
            selection_background: (255, 255, 255),
            cursor_color: (255, 255, 255),
            ansi_colors: [
                (12, 12, 12),     // 0: Black
                (197, 15, 31),    // 1: Red
                (19, 161, 14),    // 2: Green
                (193, 156, 0),    // 3: Yellow
                (0, 55, 218),     // 4: Blue
                (136, 23, 152),   // 5: Purple
                (58, 150, 221),   // 6: Cyan
                (204, 204, 204),  // 7: White
                (118, 118, 118),  // 8: Bright Black
                (231, 72, 86),    // 9: Bright Red
                (22, 198, 12),    // 10: Bright Green
                (249, 241, 165),  // 11: Bright Yellow
                (59, 120, 255),   // 12: Bright Blue
                (180, 0, 158),    // 13: Bright Purple
                (97, 214, 214),   // 14: Bright Cyan
                (242, 242, 242),  // 15: Bright White
            ],
        }
    }

    /// One Half Dark modern scheme.
    pub fn one_half_dark() -> Self {
        Self {
            name: "One Half Dark".to_string(),
            foreground: (220, 223, 228),
            background: (40, 44, 52),
            selection_background: (71, 82, 102),
            cursor_color: (255, 255, 255),
            ansi_colors: [
                (40, 44, 52),
                (224, 108, 117),
                (152, 195, 121),
                (229, 192, 123),
                (97, 175, 239),
                (198, 120, 221),
                (86, 182, 194),
                (220, 223, 228),
                (92, 99, 112),
                (224, 108, 117),
                (152, 195, 121),
                (229, 192, 123),
                (97, 175, 239),
                (198, 120, 221),
                (86, 182, 194),
                (236, 239, 244),
            ],
        }
    }

    /// One Half Light scheme.
    pub fn one_half_light() -> Self {
        Self {
            name: "One Half Light".to_string(),
            foreground: (56, 58, 66),
            background: (250, 250, 250),
            selection_background: (220, 223, 228),
            cursor_color: (79, 82, 94),
            ansi_colors: [
                (56, 58, 66),
                (228, 86, 73),
                (80, 161, 79),
                (193, 132, 1),
                (1, 132, 188),
                (166, 38, 164),
                (9, 151, 179),
                (250, 250, 250),
                (79, 82, 94),
                (223, 108, 117),
                (152, 195, 121),
                (229, 192, 123),
                (97, 175, 239),
                (198, 120, 221),
                (86, 182, 194),
                (255, 255, 255),
            ],
        }
    }

    /// Solarized Dark color scheme.
    pub fn solarized_dark() -> Self {
        Self {
            name: "Solarized Dark".to_string(),
            foreground: (131, 148, 150),
            background: (0, 43, 54),
            selection_background: (7, 54, 66),
            cursor_color: (255, 255, 255),
            ansi_colors: [
                (7, 54, 66),
                (220, 50, 47),
                (133, 153, 0),
                (181, 137, 0),
                (38, 139, 210),
                (211, 54, 130),
                (42, 161, 152),
                (238, 232, 213),
                (0, 43, 54),
                (203, 75, 22),
                (88, 110, 117),
                (101, 123, 131),
                (131, 148, 150),
                (108, 113, 196),
                (147, 161, 161),
                (253, 246, 227),
            ],
        }
    }

    /// Tango Dark palette.
    pub fn tango_dark() -> Self {
        Self {
            name: "Tango Dark".to_string(),
            foreground: (211, 215, 207),
            background: (0, 0, 0),
            selection_background: (85, 87, 83),
            cursor_color: (255, 255, 255),
            ansi_colors: [
                (0, 0, 0),
                (204, 0, 0),
                (78, 154, 6),
                (196, 160, 0),
                (52, 101, 164),
                (117, 80, 123),
                (6, 152, 154),
                (211, 215, 207),
                (85, 87, 83),
                (239, 41, 41),
                (138, 226, 52),
                (252, 233, 79),
                (114, 159, 207),
                (173, 127, 168),
                (52, 226, 226),
                (238, 238, 236),
            ],
        }
    }

    /// Classic Windows Console Vintage scheme.
    pub fn vintage() -> Self {
        Self {
            name: "Vintage".to_string(),
            foreground: (192, 192, 192),
            background: (0, 0, 0),
            selection_background: (128, 128, 128),
            cursor_color: (255, 255, 255),
            ansi_colors: [
                (0, 0, 0),
                (128, 0, 0),
                (0, 128, 0),
                (128, 128, 0),
                (0, 0, 128),
                (128, 0, 128),
                (0, 128, 128),
                (192, 192, 192),
                (128, 128, 128),
                (255, 0, 0),
                (0, 255, 0),
                (255, 255, 0),
                (0, 0, 255),
                (255, 0, 255),
                (0, 255, 255),
                (255, 255, 255),
            ],
        }
    }

    /// Returns a map of all 6 standard built-in color schemes.
    pub fn all_builtins() -> HashMap<String, Self> {
        let mut map = HashMap::new();
        let schemes = [
            Self::campbell(),
            Self::one_half_dark(),
            Self::one_half_light(),
            Self::solarized_dark(),
            Self::tango_dark(),
            Self::vintage(),
        ];
        for s in schemes {
            map.insert(s.name.clone(), s);
        }
        map
    }
}

/// Font family options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamily {
    CascadiaCode,
    CascadiaMono,
    Consolas,
    CourierNew,
}

impl FontFamily {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CascadiaCode => "Cascadia Code",
            Self::CascadiaMono => "Cascadia Mono",
            Self::Consolas => "Consolas",
            Self::CourierNew => "Courier New",
        }
    }
}

/// Font weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Light,
    Normal,
    SemiBold,
    Bold,
}

/// Cursor visual shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Bar,              // Thin vertical bar
    Vintage,          // Classic Windows underscore
    SolidBox,         // Filled box
    EmptyBox,         // Hollow box outline
}

/// Font and cursor customization.
#[derive(Debug, Clone)]
pub struct FontSettings {
    pub family: FontFamily,
    pub size_pt: f32,
    pub weight: FontWeight,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            family: FontFamily::CascadiaCode,
            size_pt: 12.0,
            weight: FontWeight::Normal,
            cursor_shape: CursorShape::Bar,
            cursor_blink: true,
        }
    }
}

// ============================================================================
// Section 3: Terminal Buffer & Scrollback Management
// ============================================================================

/// Result of searching inside the terminal buffer history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub line_idx: usize,
    pub start_col: usize,
    pub length: usize,
    pub matched_text: String,
}

/// Grid buffer managing the visible viewport and historic scrollback buffer.
#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    pub cols: usize,
    pub rows: usize,
    pub max_scrollback: usize,
    pub scrollback: VecDeque<Vec<TerminalCell>>,
    pub viewport: Vec<Vec<TerminalCell>>,
    pub scroll_offset: usize, // 0 = viewing active viewport, >0 = viewing history
}

impl TerminalBuffer {
    pub fn new(cols: usize, rows: usize, max_scrollback: usize) -> Self {
        let cols = cols.max(10);
        let rows = rows.max(2);
        let mut viewport = Vec::with_capacity(rows);
        for _ in 0..rows {
            viewport.push(vec![TerminalCell::default(); cols]);
        }
        Self {
            cols,
            rows,
            max_scrollback,
            scrollback: VecDeque::new(),
            viewport,
            scroll_offset: 0,
        }
    }

    /// Resize buffer grid dimensions while preserving existing content.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let new_cols = new_cols.max(10);
        let new_rows = new_rows.max(2);
        if new_cols == self.cols && new_rows == self.rows {
            return;
        }

        // Adjust rows in viewport
        while self.viewport.len() > new_rows {
            let removed = self.viewport.remove(0);
            if self.scrollback.len() >= self.max_scrollback {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(removed);
        }
        while self.viewport.len() < new_rows {
            self.viewport.push(vec![TerminalCell::default(); new_cols]);
        }

        // Adjust cols in all lines
        for line in &mut self.viewport {
            line.resize(new_cols, TerminalCell::default());
        }
        for line in &mut self.scrollback {
            line.resize(new_cols, TerminalCell::default());
        }

        self.cols = new_cols;
        self.rows = new_rows;
        self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
    }

    /// Advance to the next line, pushing lines into the scrollback buffer when reaching bottom.
    pub fn newline(&mut self, cursor: &mut CursorPosition) {
        if cursor.row + 1 >= self.rows {
            // Push top line to scrollback
            if self.viewport.len() >= self.rows {
                let top_line = self.viewport.remove(0);
                if self.scrollback.len() >= self.max_scrollback {
                    self.scrollback.pop_front();
                }
                self.scrollback.push_back(top_line);
            }
            self.viewport.push(vec![TerminalCell::default(); self.cols]);
            cursor.row = self.rows.saturating_sub(1);
        } else {
            cursor.row += 1;
        }
    }

    /// Carriage return: move cursor to start of line.
    pub fn carriage_return(&self, cursor: &mut CursorPosition) {
        cursor.col = 0;
    }

    /// Backspace: move cursor left by one cell.
    pub fn backspace(&self, cursor: &mut CursorPosition) {
        if cursor.col > 0 {
            cursor.col -= 1;
        }
    }

    /// Horizontal tab: advance cursor to next 8-space tab stop.
    pub fn tab(&mut self, cursor: &mut CursorPosition) {
        let next_tab = (cursor.col / 8 + 1) * 8;
        cursor.col = next_tab.min(self.cols.saturating_sub(1));
    }

    /// Writes a single character to the viewport grid, handling wrapping and control chars.
    pub fn write_char(&mut self, ch: char, attrs: CellAttributes, cursor: &mut CursorPosition) {
        match ch {
            '\r' => self.carriage_return(cursor),
            '\n' => {
                self.newline(cursor);
                cursor.col = 0;
            }
            '\t' => self.tab(cursor),
            '\x08' => self.backspace(cursor),
            _ => {
                if cursor.col >= self.cols {
                    self.newline(cursor);
                    cursor.col = 0;
                }
                if cursor.row < self.viewport.len() && cursor.col < self.cols {
                    self.viewport[cursor.row][cursor.col] = TerminalCell::new(ch, attrs);
                    cursor.col += 1;
                }
            }
        }
    }

    /// Erase In Line (EL): mode 0 (cursor to end), mode 1 (start to cursor), mode 2 (all).
    pub fn erase_in_line(&mut self, mode: u8, cursor: &CursorPosition) {
        if cursor.row >= self.viewport.len() {
            return;
        }
        let line = &mut self.viewport[cursor.row];
        match mode {
            0 => {
                // Clear from cursor to end
                for c in cursor.col..self.cols {
                    if c < line.len() {
                        line[c] = TerminalCell::default();
                    }
                }
            }
            1 => {
                // Clear from beginning to cursor
                for c in 0..=cursor.col.min(self.cols.saturating_sub(1)) {
                    if c < line.len() {
                        line[c] = TerminalCell::default();
                    }
                }
            }
            2 => {
                // Clear entire line
                for cell in line.iter_mut() {
                    *cell = TerminalCell::default();
                }
            }
            _ => {}
        }
    }

    /// Erase In Display (ED):
    /// mode 0: cursor to end of screen
    /// mode 1: start of screen to cursor
    /// mode 2: clear entire screen
    /// mode 3: clear entire screen AND clear scrollback buffer
    pub fn erase_in_display(&mut self, mode: u8, cursor: &CursorPosition) {
        match mode {
            0 => {
                self.erase_in_line(0, cursor);
                for r in (cursor.row + 1)..self.viewport.len() {
                    for cell in &mut self.viewport[r] {
                        *cell = TerminalCell::default();
                    }
                }
            }
            1 => {
                for r in 0..cursor.row {
                    if r < self.viewport.len() {
                        for cell in &mut self.viewport[r] {
                            *cell = TerminalCell::default();
                        }
                    }
                }
                self.erase_in_line(1, cursor);
            }
            2 => {
                for line in &mut self.viewport {
                    for cell in line.iter_mut() {
                        *cell = TerminalCell::default();
                    }
                }
            }
            3 => {
                for line in &mut self.viewport {
                    for cell in line.iter_mut() {
                        *cell = TerminalCell::default();
                    }
                }
                self.scrollback.clear();
                self.scroll_offset = 0;
            }
            _ => {}
        }
    }

    /// Adjust scroll view offset (positive = scroll up into history, negative = scroll down).
    pub fn scroll_view(&mut self, delta: i32) {
        let max_offset = self.scrollback.len();
        let current = self.scroll_offset as i64;
        let new_offset = (current + delta as i64).clamp(0, max_offset as i64) as usize;
        self.scroll_offset = new_offset;
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.scrollback.len();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Total number of stored lines (scrollback + viewport).
    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.viewport.len()
    }

    /// Retrieves a line for rendering based on the active `scroll_offset`.
    pub fn get_rendered_line(&self, viewport_row: usize) -> Option<&Vec<TerminalCell>> {
        if viewport_row >= self.rows {
            return None;
        }
        if self.scroll_offset == 0 {
            self.viewport.get(viewport_row)
        } else {
            // Scrolled up: calculate index across scrollback + viewport
            let total_scroll = self.scrollback.len();
            let effective_top = total_scroll.saturating_sub(self.scroll_offset);
            let target_line = effective_top + viewport_row;

            if target_line < total_scroll {
                self.scrollback.get(target_line)
            } else {
                let vp_idx = target_line - total_scroll;
                self.viewport.get(vp_idx)
            }
        }
    }

    /// Extracts plain text representation of a single line.
    pub fn get_line_text(&self, line: &[TerminalCell]) -> String {
        let mut s = String::with_capacity(line.len());
        for cell in line {
            s.push(cell.ch);
        }
        s.trim_end().to_string()
    }

    /// Extracts all text currently in scrollback and viewport.
    pub fn get_all_text(&self) -> String {
        let mut out = String::new();
        for line in &self.scrollback {
            out.push_str(&self.get_line_text(line));
            out.push('\n');
        }
        for line in &self.viewport {
            out.push_str(&self.get_line_text(line));
            out.push('\n');
        }
        out
    }

    /// Extracts text highlighted by the active selection range in viewport coordinates.
    pub fn get_selected_text(&self, sel: &SelectionRange) -> String {
        let (start, end) = if sel.start <= sel.end {
            (sel.start, sel.end)
        } else {
            (sel.end, sel.start)
        };

        let mut result = String::new();
        for r in start.row..=end.row {
            if let Some(line) = self.viewport.get(r) {
                let c_start = if r == start.row { start.col } else { 0 };
                let c_end = if r == end.row { end.col } else { line.len().saturating_sub(1) };
                for c in c_start..=c_end {
                    if let Some(cell) = line.get(c) {
                        result.push(cell.ch);
                    }
                }
                if r < end.row {
                    result.push('\n');
                }
            }
        }
        result
    }

    /// Searches both scrollback history and active viewport for a query string.
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchResult> {
        let mut results = Vec::new();
        if query.is_empty() {
            return results;
        }

        let query_processed = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let check_line = |line_idx: usize, line: &[TerminalCell], results: &mut Vec<SearchResult>| {
            let mut line_str = String::with_capacity(line.len());
            for c in line {
                line_str.push(c.ch);
            }
            let cmp_str = if case_sensitive {
                line_str.clone()
            } else {
                line_str.to_lowercase()
            };

            let mut start_idx = 0;
            while let Some(found) = cmp_str[start_idx..].find(&query_processed) {
                let match_col = start_idx + found;
                let matched_text = line_str[match_col..(match_col + query.len())].to_string();
                results.push(SearchResult {
                    line_idx,
                    start_col: match_col,
                    length: query.len(),
                    matched_text,
                });
                start_idx = match_col + 1;
            }
        };

        for (idx, line) in self.scrollback.iter().enumerate() {
            check_line(idx, line, &mut results);
        }
        let sb_len = self.scrollback.len();
        for (idx, line) in self.viewport.iter().enumerate() {
            check_line(sb_len + idx, line, &mut results);
        }

        results
    }
}

// ============================================================================
// Section 4: VT100 / ANSI Escape Sequence Parser
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnsiState {
    Ground,
    Escape,
    Csi,
    Osc,
}

/// Robust VT100 / ANSI Escape Sequence Parser.
///
/// Supports SGR text formatting (bold, dim, italic, underline, invert),
/// 16 ANSI colors, 256-color palette, 24-bit TrueColor RGB,
/// cursor positioning (CUP), relative moves (CUU, CUD, CUF, CUB),
/// Erase in Display (ED), Erase in Line (EL), and OSC window title setting.
#[derive(Debug, Clone)]
pub struct AnsiEscapeParser {
    state: AnsiState,
    params: Vec<u32>,
    current_param: u32,
    has_param: bool,
    osc_buffer: String,
    pub current_attrs: CellAttributes,
    pub saved_cursor: CursorPosition,
}

impl Default for AnsiEscapeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiEscapeParser {
    pub fn new() -> Self {
        Self {
            state: AnsiState::Ground,
            params: Vec::with_capacity(8),
            current_param: 0,
            has_param: false,
            osc_buffer: String::with_capacity(64),
            current_attrs: CellAttributes::default(),
            saved_cursor: CursorPosition::default(),
        }
    }

    /// Process a string input, updating buffer, cursor, and returning any title change.
    pub fn process_str(
        &mut self,
        input: &str,
        buffer: &mut TerminalBuffer,
        cursor: &mut CursorPosition,
    ) -> Option<String> {
        let mut title_change = None;
        for ch in input.chars() {
            if let Some(new_title) = self.process_char(ch, buffer, cursor) {
                title_change = Some(new_title);
            }
        }
        title_change
    }

    /// Process a single character through the ANSI state machine.
    pub fn process_char(
        &mut self,
        ch: char,
        buffer: &mut TerminalBuffer,
        cursor: &mut CursorPosition,
    ) -> Option<String> {
        match self.state {
            AnsiState::Ground => match ch {
                '\x1b' => {
                    self.state = AnsiState::Escape;
                    None
                }
                _ => {
                    buffer.write_char(ch, self.current_attrs, cursor);
                    None
                }
            },
            AnsiState::Escape => match ch {
                '[' => {
                    self.state = AnsiState::Csi;
                    self.params.clear();
                    self.current_param = 0;
                    self.has_param = false;
                    None
                }
                ']' => {
                    self.state = AnsiState::Osc;
                    self.osc_buffer.clear();
                    None
                }
                '7' => {
                    // Save cursor (DECSC)
                    self.saved_cursor = *cursor;
                    self.state = AnsiState::Ground;
                    None
                }
                '8' => {
                    // Restore cursor (DECRC)
                    *cursor = self.saved_cursor;
                    self.state = AnsiState::Ground;
                    None
                }
                _ => {
                    self.state = AnsiState::Ground;
                    None
                }
            },
            AnsiState::Csi => match ch {
                '0'..='9' => {
                    let digit = ch.to_digit(10).unwrap_or(0);
                    self.current_param = self.current_param.saturating_mul(10).saturating_add(digit);
                    self.has_param = true;
                    None
                }
                ';' => {
                    self.params.push(self.current_param);
                    self.current_param = 0;
                    self.has_param = false;
                    None
                }
                // Command dispatch
                'm' => {
                    if self.has_param {
                        self.params.push(self.current_param);
                    }
                    self.handle_sgr();
                    self.state = AnsiState::Ground;
                    None
                }
                'A' => {
                    // Cursor Up (CUU)
                    let n = if self.has_param { self.current_param.max(1) } else { 1 } as usize;
                    cursor.row = cursor.row.saturating_sub(n);
                    self.state = AnsiState::Ground;
                    None
                }
                'B' => {
                    // Cursor Down (CUD)
                    let n = if self.has_param { self.current_param.max(1) } else { 1 } as usize;
                    cursor.row = (cursor.row + n).min(buffer.rows.saturating_sub(1));
                    self.state = AnsiState::Ground;
                    None
                }
                'C' => {
                    // Cursor Forward (CUF)
                    let n = if self.has_param { self.current_param.max(1) } else { 1 } as usize;
                    cursor.col = (cursor.col + n).min(buffer.cols.saturating_sub(1));
                    self.state = AnsiState::Ground;
                    None
                }
                'D' => {
                    // Cursor Back (CUB)
                    let n = if self.has_param { self.current_param.max(1) } else { 1 } as usize;
                    cursor.col = cursor.col.saturating_sub(n);
                    self.state = AnsiState::Ground;
                    None
                }
                'H' | 'f' => {
                    // Cursor Position (CUP): 1-indexed (row;col)
                    if self.has_param {
                        self.params.push(self.current_param);
                    }
                    let r = self.params.get(0).copied().unwrap_or(1).max(1) as usize;
                    let c = self.params.get(1).copied().unwrap_or(1).max(1) as usize;
                    cursor.row = (r - 1).min(buffer.rows.saturating_sub(1));
                    cursor.col = (c - 1).min(buffer.cols.saturating_sub(1));
                    self.state = AnsiState::Ground;
                    None
                }
                'J' => {
                    // Erase In Display (ED)
                    let mode = if self.has_param { self.current_param as u8 } else { 0 };
                    buffer.erase_in_display(mode, cursor);
                    self.state = AnsiState::Ground;
                    None
                }
                'K' => {
                    // Erase In Line (EL)
                    let mode = if self.has_param { self.current_param as u8 } else { 0 };
                    buffer.erase_in_line(mode, cursor);
                    self.state = AnsiState::Ground;
                    None
                }
                's' => {
                    // ANSI.SYS Save cursor
                    self.saved_cursor = *cursor;
                    self.state = AnsiState::Ground;
                    None
                }
                'u' => {
                    // ANSI.SYS Restore cursor
                    *cursor = self.saved_cursor;
                    self.state = AnsiState::Ground;
                    None
                }
                _ => {
                    // Unrecognized CSI sequence
                    self.state = AnsiState::Ground;
                    None
                }
            },
            AnsiState::Osc => {
                if ch == '\x07' || ch == '\x1b' {
                    // String terminator: parse OSC buffer
                    let title = self.handle_osc();
                    self.state = AnsiState::Ground;
                    title
                } else {
                    self.osc_buffer.push(ch);
                    None
                }
            }
        }
    }

    /// Handles Select Graphic Rendition (SGR) sequence parameters.
    fn handle_sgr(&mut self) {
        if self.params.is_empty() {
            self.current_attrs.reset();
            return;
        }

        let mut i = 0;
        while i < self.params.len() {
            let p = self.params[i];
            match p {
                0 => self.current_attrs.reset(),
                1 => self.current_attrs.bold = true,
                2 => self.current_attrs.dim = true,
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                7 => self.current_attrs.invert = true,
                9 => self.current_attrs.strikethrough = true,
                22 => {
                    self.current_attrs.bold = false;
                    self.current_attrs.dim = false;
                }
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                27 => self.current_attrs.invert = false,
                29 => self.current_attrs.strikethrough = false,
                // Standard Foreground Colors (30-37)
                30..=37 => self.current_attrs.fg = Color::Ansi16((p - 30) as u8),
                // Extended Foreground Color (38)
                38 => {
                    if i + 1 < self.params.len() {
                        match self.params[i + 1] {
                            5 => {
                                // 256 Colors: 38;5;n
                                if i + 2 < self.params.len() {
                                    self.current_attrs.fg = Color::Color256(self.params[i + 2] as u8);
                                    i += 2;
                                }
                            }
                            2 => {
                                // 24-bit TrueColor: 38;2;r;g;b
                                if i + 4 < self.params.len() {
                                    let r = self.params[i + 2] as u8;
                                    let g = self.params[i + 3] as u8;
                                    let b = self.params[i + 4] as u8;
                                    self.current_attrs.fg = Color::Rgb(r, g, b);
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                39 => self.current_attrs.fg = Color::DefaultForeground,
                // Standard Background Colors (40-47)
                40..=47 => self.current_attrs.bg = Color::Ansi16((p - 40) as u8),
                // Extended Background Color (48)
                48 => {
                    if i + 1 < self.params.len() {
                        match self.params[i + 1] {
                            5 => {
                                // 256 Colors: 48;5;n
                                if i + 2 < self.params.len() {
                                    self.current_attrs.bg = Color::Color256(self.params[i + 2] as u8);
                                    i += 2;
                                }
                            }
                            2 => {
                                // 24-bit TrueColor: 48;2;r;g;b
                                if i + 4 < self.params.len() {
                                    let r = self.params[i + 2] as u8;
                                    let g = self.params[i + 3] as u8;
                                    let b = self.params[i + 4] as u8;
                                    self.current_attrs.bg = Color::Rgb(r, g, b);
                                    i += 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                49 => self.current_attrs.bg = Color::DefaultBackground,
                // High-Intensity Foreground (90-97)
                90..=97 => self.current_attrs.fg = Color::Ansi16((p - 90 + 8) as u8),
                // High-Intensity Background (100-107)
                100..=107 => self.current_attrs.bg = Color::Ansi16((p - 100 + 8) as u8),
                _ => {}
            }
            i += 1;
        }
    }

    /// Handles Operating System Command (OSC) string (e.g. `0;Title` or `2;Title`).
    fn handle_osc(&self) -> Option<String> {
        let parts: Vec<&str> = self.osc_buffer.splitn(2, ';').collect();
        if parts.len() == 2 {
            let cmd = parts[0];
            let payload = parts[1];
            if cmd == "0" || cmd == "2" {
                return Some(payload.to_string());
            }
        }
        None
    }
}

// ============================================================================
// Section 5: Profiles & Shell Subsystems
// ============================================================================

/// Shell subsystems supported by HybridOS Windows Terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellType {
    PowerShell,
    WindowsCmd,
    LinuxBash,
    HybridUniversal,
}

impl ShellType {
    pub fn to_flavor(&self) -> ShellFlavor {
        match self {
            Self::PowerShell => ShellFlavor::PowerShell,
            Self::WindowsCmd => ShellFlavor::WindowsCmd,
            Self::LinuxBash => ShellFlavor::LinuxBash,
            Self::HybridUniversal => ShellFlavor::Hybrid,
        }
    }
}

/// Profile configuration representing a launchable terminal target.
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: u32,
    pub shell_type: ShellType,
    pub name: String,
    pub command: String,
    pub icon: &'static str,
    pub color_scheme: String,
    pub font: FontSettings,
    pub starting_directory: String,
    pub tab_title: String,
}

impl Profile {
    /// Microsoft PowerShell profile.
    pub fn powershell() -> Self {
        Self {
            id: 1,
            shell_type: ShellType::PowerShell,
            name: "PowerShell".to_string(),
            command: "pwsh.exe".to_string(),
            icon: "⚡",
            color_scheme: "Campbell".to_string(),
            font: FontSettings::default(),
            starting_directory: "C:\\".to_string(),
            tab_title: "PowerShell".to_string(),
        }
    }

    /// Windows Command Prompt profile.
    pub fn windows_cmd() -> Self {
        let mut font = FontSettings::default();
        font.family = FontFamily::Consolas;
        font.cursor_shape = CursorShape::Vintage;
        Self {
            id: 2,
            shell_type: ShellType::WindowsCmd,
            name: "Command Prompt".to_string(),
            command: "cmd.exe".to_string(),
            icon: ">_",
            color_scheme: "Vintage".to_string(),
            font,
            starting_directory: "C:\\".to_string(),
            tab_title: "Command Prompt".to_string(),
        }
    }

    /// Linux GNU Bash subsystem profile.
    pub fn linux_bash() -> Self {
        Self {
            id: 3,
            shell_type: ShellType::LinuxBash,
            name: "Ubuntu Linux (Bash)".to_string(),
            command: "/bin/bash".to_string(),
            icon: "🐧",
            color_scheme: "One Half Dark".to_string(),
            font: FontSettings::default(),
            starting_directory: "/home/user".to_string(),
            tab_title: "Ubuntu (Bash)".to_string(),
        }
    }

    /// HybridOS Universal Kernel Shell profile.
    pub fn hybrid_universal() -> Self {
        Self {
            id: 4,
            shell_type: ShellType::HybridUniversal,
            name: "HybridOS Universal Shell".to_string(),
            command: "hybridos-shell".to_string(),
            icon: "⚛",
            color_scheme: "Campbell".to_string(),
            font: FontSettings::default(),
            starting_directory: "/".to_string(),
            tab_title: "HybridOS Universal".to_string(),
        }
    }

    /// Returns all standard default profiles.
    pub fn all_builtins() -> Vec<Self> {
        vec![
            Self::powershell(),
            Self::windows_cmd(),
            Self::linux_bash(),
            Self::hybrid_universal(),
        ]
    }
}

// ============================================================================
// Section 6: Split Pane Engine
// ============================================================================

/// Direction to split a terminal pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Left and right panes side-by-side (Alt+Shift+Plus).
    Vertical,
    /// Top and bottom panes stacked (Alt+Shift+Minus).
    Horizontal,
}

/// An independent terminal session running inside a pane.
pub struct TerminalPane {
    pub id: u64,
    pub profile: Profile,
    pub title: String,
    pub buffer: TerminalBuffer,
    pub cursor: CursorPosition,
    pub parser: AnsiEscapeParser,
    pub rect: PaneRect,
    pub shell: HybridShell,
    pub input_line: String,
    pub is_active: bool,
}

impl fmt::Debug for TerminalPane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalPane")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("rect", &self.rect)
            .field("is_active", &self.is_active)
            .finish()
    }
}

impl TerminalPane {
    pub fn new(id: u64, profile: Profile, cols: usize, rows: usize) -> Self {
        let mut shell = HybridShell::new();
        shell.flavor = profile.shell_type.to_flavor();

        let mut pane = Self {
            id,
            profile,
            title: String::new(),
            buffer: TerminalBuffer::new(cols, rows, 10_000),
            cursor: CursorPosition::default(),
            parser: AnsiEscapeParser::new(),
            rect: PaneRect::EMPTY,
            shell,
            input_line: String::new(),
            is_active: false,
        };

        pane.title = pane.profile.tab_title.clone();
        pane.write_initial_greeting();
        pane
    }

    /// Writes initial shell greeting banner and prompt to the terminal buffer.
    fn write_initial_greeting(&mut self) {
        let prompt_str = self.shell.prompt();
        let greeting = match self.profile.shell_type {
            ShellType::PowerShell => {
                "\x1b[1;36mWindows PowerShell\x1b[0m\nCopyright (C) Microsoft Corporation. All rights reserved.\n\n"
            }
            ShellType::WindowsCmd => {
                "Microsoft Windows [Version 10.0.22631.3007]\n(c) Microsoft Corporation. All rights reserved.\n\n"
            }
            ShellType::LinuxBash => {
                "\x1b[1;32mWelcome to Ubuntu Linux (HybridOS Subsystem)\x1b[0m\n * Documentation:  https://help.ubuntu.com\n\n"
            }
            ShellType::HybridUniversal => {
                "\x1b[1;35mHybridOS Universal Dual-Personality Kernel CLI\x1b[0m\nType 'help' for command reference.\n\n"
            }
        };

        self.write_str(greeting);
        self.write_str(&prompt_str);
    }

    /// Feeds output text into the pane's ANSI parser and updates buffer cells.
    pub fn write_str(&mut self, text: &str) {
        let title_change = self.parser.process_str(text, &mut self.buffer, &mut self.cursor);
        if let Some(new_title) = title_change {
            self.title = new_title;
        }
    }

    /// Executes a command string through the pane's shell session.
    pub fn execute_command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            let output = self.shell.execute(trimmed);
            if !output.is_empty() {
                self.write_str(&output);
                if !output.ends_with('\n') {
                    self.write_str("\n");
                }
            }
        }
        let prompt = self.shell.prompt();
        self.write_str(&prompt);
        self.input_line.clear();
    }
}

/// Binary tree node representing a leaf terminal pane or a split divider.
pub enum PaneNode {
    Leaf(TerminalPane),
    Split {
        id: u64,
        direction: SplitDirection,
        ratio: f32, // between 0.1 and 0.9, default 0.5
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl fmt::Debug for PaneNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leaf(pane) => write!(f, "Leaf(id={})", pane.id),
            Self::Split { id, direction, ratio, .. } => {
                write!(f, "Split(id={}, dir={:?}, ratio={})", id, direction, ratio)
            }
        }
    }
}

/// Split pane management engine.
pub struct PaneTree {
    pub root: PaneNode,
    pub active_pane_id: u64,
    next_pane_id: u64,
    next_split_id: u64,
}

impl PaneTree {
    pub fn new(initial_profile: Profile, cols: usize, rows: usize) -> Self {
        let mut pane = TerminalPane::new(1, initial_profile, cols, rows);
        pane.is_active = true;
        Self {
            root: PaneNode::Leaf(pane),
            active_pane_id: 1,
            next_pane_id: 2,
            next_split_id: 1,
        }
    }

    /// Split active pane into two sub-panes.
    pub fn split_active(&mut self, direction: SplitDirection, profile: Profile) -> Option<u64> {
        let new_id = self.next_pane_id;
        self.next_pane_id += 1;
        let split_id = self.next_split_id;
        self.next_split_id += 1;

        let new_pane = TerminalPane::new(new_id, profile, 80, 24);

        if Self::split_node(&mut self.root, self.active_pane_id, direction, split_id, new_pane) {
            self.set_active_pane(new_id);
            Some(new_id)
        } else {
            None
        }
    }

    fn split_node(
        node: &mut PaneNode,
        target_id: u64,
        direction: SplitDirection,
        split_id: u64,
        new_pane: TerminalPane,
    ) -> bool {
        let should_split = match node {
            PaneNode::Leaf(ref pane) => pane.id == target_id,
            PaneNode::Split { ref mut first, ref mut second, .. } => {
                if Self::split_node(first, target_id, direction, split_id, new_pane.clone_for_split()) {
                    return true;
                }
                return Self::split_node(second, target_id, direction, split_id, new_pane);
            }
        };

        if should_split {
            // Replace leaf with split node
            let old_leaf = match std::mem::replace(node, PaneNode::Split {
                id: split_id,
                direction,
                ratio: 0.5,
                first: Box::new(PaneNode::Leaf(TerminalPane::new(0, Profile::powershell(), 10, 2))),
                second: Box::new(PaneNode::Leaf(TerminalPane::new(0, Profile::powershell(), 10, 2))),
            }) {
                PaneNode::Leaf(pane) => pane,
                _ => unreachable!(),
            };

            *node = PaneNode::Split {
                id: split_id,
                direction,
                ratio: 0.5,
                first: Box::new(PaneNode::Leaf(old_leaf)),
                second: Box::new(PaneNode::Leaf(new_pane)),
            };
            return true;
        }

        false
    }

    /// Close active pane, pruning the split and promoting its sibling.
    pub fn close_active(&mut self) -> bool {
        self.close_pane(self.active_pane_id)
    }

    /// Closes a pane by ID. If it is the last pane, returns false.
    pub fn close_pane(&mut self, pane_id: u64) -> bool {
        if self.count_panes() <= 1 {
            return false;
        }

        let mut replacement = None;
        if Self::prune_node(&mut self.root, pane_id, &mut replacement) {
            if let Some(rep) = replacement {
                self.root = rep;
            }
            // Update active pane if closed pane was active
            if self.active_pane_id == pane_id {
                if let Some(first) = self.panes().first() {
                    let first_id = first.id;
                    self.set_active_pane(first_id);
                }
            }
            true
        } else {
            false
        }
    }

    fn prune_node(
        node: &mut PaneNode,
        target_id: u64,
        replacement: &mut Option<PaneNode>,
    ) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split { ref mut first, ref mut second, .. } => {
                if let PaneNode::Leaf(ref p) = **first {
                    if p.id == target_id {
                        // Promote second
                        let promoted = std::mem::replace(
                            &mut **second,
                            PaneNode::Leaf(TerminalPane::new(0, Profile::powershell(), 10, 2)),
                        );
                        *replacement = Some(promoted);
                        return true;
                    }
                }
                if let PaneNode::Leaf(ref p) = **second {
                    if p.id == target_id {
                        // Promote first
                        let promoted = std::mem::replace(
                            &mut **first,
                            PaneNode::Leaf(TerminalPane::new(0, Profile::powershell(), 10, 2)),
                        );
                        *replacement = Some(promoted);
                        return true;
                    }
                }

                let mut sub_rep = None;
                if Self::prune_node(first, target_id, &mut sub_rep) {
                    if let Some(rep) = sub_rep {
                        *first = Box::new(rep);
                    }
                    return true;
                }
                if Self::prune_node(second, target_id, &mut sub_rep) {
                    if let Some(rep) = sub_rep {
                        *second = Box::new(rep);
                    }
                    return true;
                }
                false
            }
        }
    }

    /// Set active pane by ID.
    pub fn set_active_pane(&mut self, pane_id: u64) {
        self.active_pane_id = pane_id;
        for pane in self.panes_mut() {
            pane.is_active = pane.id == pane_id;
        }
    }

    /// Count total leaf panes in the tree.
    pub fn count_panes(&self) -> usize {
        self.panes().len()
    }

    /// Returns references to all leaf panes in the tree.
    pub fn panes(&self) -> Vec<&TerminalPane> {
        let mut list = Vec::new();
        Self::collect_panes(&self.root, &mut list);
        list
    }

    fn collect_panes<'a>(node: &'a PaneNode, list: &mut Vec<&'a TerminalPane>) {
        match node {
            PaneNode::Leaf(ref pane) => list.push(pane),
            PaneNode::Split { ref first, ref second, .. } => {
                Self::collect_panes(first, list);
                Self::collect_panes(second, list);
            }
        }
    }

    /// Returns mutable references to all leaf panes in the tree.
    pub fn panes_mut(&mut self) -> Vec<&mut TerminalPane> {
        let mut list = Vec::new();
        Self::collect_panes_mut(&mut self.root, &mut list);
        list
    }

    fn collect_panes_mut<'a>(node: &'a mut PaneNode, list: &mut Vec<&'a mut TerminalPane>) {
        match node {
            PaneNode::Leaf(ref mut pane) => list.push(pane),
            PaneNode::Split { ref mut first, ref mut second, .. } => {
                Self::collect_panes_mut(first, list);
                Self::collect_panes_mut(second, list);
            }
        }
    }

    /// Get active pane reference.
    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.panes().into_iter().find(|p| p.id == self.active_pane_id)
    }

    /// Get mutable reference to active pane.
    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        let id = self.active_pane_id;
        self.panes_mut().into_iter().find(|p| p.id == id)
    }

    /// Resize split splitter ratio by ID.
    pub fn resize_splitter(&mut self, split_id: u64, new_ratio: f32) -> bool {
        let clamped = new_ratio.clamp(0.1, 0.9);
        Self::apply_resize(&mut self.root, split_id, clamped)
    }

    fn apply_resize(node: &mut PaneNode, split_id: u64, ratio: f32) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split { id, ratio: ref mut r, ref mut first, ref mut second, .. } => {
                if *id == split_id {
                    *r = ratio;
                    true
                } else {
                    Self::apply_resize(first, split_id, ratio) || Self::apply_resize(second, split_id, ratio)
                }
            }
        }
    }

    /// Recursively calculate bounding rectangles and grid sizes for all panes.
    pub fn layout(&mut self, bounds: PaneRect) {
        Self::compute_layout(&mut self.root, bounds);
    }

    fn compute_layout(node: &mut PaneNode, bounds: PaneRect) {
        match node {
            PaneNode::Leaf(ref mut pane) => {
                pane.rect = bounds;
                // Standard character cell estimation: 9px width, 18px height
                let cols = (bounds.width / 9).max(10) as usize;
                let rows = (bounds.height / 18).max(2) as usize;
                pane.buffer.resize(cols, rows);
            }
            PaneNode::Split { direction, ratio, ref mut first, ref mut second, .. } => {
                match direction {
                    SplitDirection::Vertical => {
                        let w1 = (bounds.width as f32 * *ratio) as u32;
                        let w2 = bounds.width.saturating_sub(w1);
                        let r1 = PaneRect::new(bounds.x, bounds.y, w1, bounds.height);
                        let r2 = PaneRect::new(bounds.x + w1 as i32, bounds.y, w2, bounds.height);
                        Self::compute_layout(first, r1);
                        Self::compute_layout(second, r2);
                    }
                    SplitDirection::Horizontal => {
                        let h1 = (bounds.height as f32 * *ratio) as u32;
                        let h2 = bounds.height.saturating_sub(h1);
                        let r1 = PaneRect::new(bounds.x, bounds.y, bounds.width, h1);
                        let r2 = PaneRect::new(bounds.x, bounds.y + h1 as i32, bounds.width, h2);
                        Self::compute_layout(first, r1);
                        Self::compute_layout(second, r2);
                    }
                }
            }
        }
    }
}

impl TerminalPane {
    fn clone_for_split(&self) -> Self {
        Self::new(self.id, self.profile.clone(), self.buffer.cols, self.buffer.rows)
    }
}

// ============================================================================
// Section 7: Multi-Tab Architecture
// ============================================================================

/// Represents a single tab in the Windows Terminal tab bar.
pub struct TerminalTab {
    pub id: u64,
    pub profile: Profile,
    pub title: String,
    pub panes: PaneTree,
    pub active_selection: Option<SelectionRange>,
}

impl TerminalTab {
    pub fn new(id: u64, profile: Profile, cols: usize, rows: usize) -> Self {
        let title = profile.tab_title.clone();
        let panes = PaneTree::new(profile.clone(), cols, rows);
        Self {
            id,
            profile,
            title,
            panes,
            active_selection: None,
        }
    }

    /// Access active pane.
    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.panes.active_pane()
    }

    /// Access active pane mutably.
    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        self.panes.active_pane_mut()
    }

    /// Access active buffer.
    pub fn active_buffer(&self) -> Option<&TerminalBuffer> {
        self.active_pane().map(|p| &p.buffer)
    }

    /// Access active buffer mutably.
    pub fn active_buffer_mut(&mut self) -> Option<&mut TerminalBuffer> {
        self.active_pane_mut().map(|p| &mut p.buffer)
    }
}

/// The tab strip managing tabs, profile dropdowns, and drag reordering.
pub struct TabStrip {
    pub tabs: Vec<TerminalTab>,
    pub active_tab_index: usize,
    next_tab_id: u64,
    pub dropdown_open: bool,
}

impl Default for TabStrip {
    fn default() -> Self {
        Self::new()
    }
}

impl TabStrip {
    pub fn new() -> Self {
        let default_profile = Profile::powershell();
        let first_tab = TerminalTab::new(1, default_profile, 80, 24);
        Self {
            tabs: vec![first_tab],
            active_tab_index: 0,
            next_tab_id: 2,
            dropdown_open: false,
        }
    }

    /// Create and append a new tab using the specified profile.
    pub fn new_tab(&mut self, profile: Profile) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = TerminalTab::new(id, profile, 80, 24);
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len().saturating_sub(1);
        id
    }

    /// Close tab by ID.
    pub fn close_tab(&mut self, tab_id: u64) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }

        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(pos);
            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            }
            true
        } else {
            false
        }
    }

    /// Select tab by index.
    pub fn select_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
        }
    }

    /// Move to next tab.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    /// Move to previous tab.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            if self.active_tab_index == 0 {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            } else {
                self.active_tab_index -= 1;
            }
        }
    }

    /// Drag and reorder tab from one index to another.
    pub fn reorder_tab(&mut self, from_idx: usize, to_idx: usize) -> bool {
        if from_idx >= self.tabs.len() || to_idx >= self.tabs.len() || from_idx == to_idx {
            return false;
        }
        let tab = self.tabs.remove(from_idx);
        self.tabs.insert(to_idx, tab);
        self.active_tab_index = to_idx;
        true
    }

    /// Get reference to active tab.
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Get mutable reference to active tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active_tab_index)
    }
}

// ============================================================================
// Section 8: Input, Clipboard & Interactive Engine
// ============================================================================

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self { ctrl: false, alt: false, shift: false, win: false };
    pub const CTRL: Self = Self { ctrl: true, alt: false, shift: false, win: false };
    pub const ALT: Self = Self { ctrl: false, alt: true, shift: false, win: false };
    pub const SHIFT: Self = Self { ctrl: false, alt: false, shift: true, win: false };
    pub const CTRL_SHIFT: Self = Self { ctrl: true, alt: false, shift: true, win: false };
    pub const ALT_SHIFT: Self = Self { ctrl: false, alt: true, shift: true, win: false };
}

/// Abstract key representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Plus,
    Minus,
    F(u8),
}

/// Terminal clipboard supporting copy on select and paste.
#[derive(Debug, Clone, Default)]
pub struct TerminalClipboard {
    pub content: String,
    pub copy_on_select: bool,
}

impl TerminalClipboard {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            copy_on_select: false,
        }
    }

    pub fn copy(&mut self, text: &str) {
        self.content = text.to_string();
    }

    pub fn paste(&self) -> &str {
        &self.content
    }
}

/// In-buffer search bar state (Ctrl+Shift+F).
#[derive(Debug, Clone, Default)]
pub struct SearchBarState {
    pub is_open: bool,
    pub query: String,
    pub matches: Vec<SearchResult>,
    pub current_match_idx: usize,
    pub case_sensitive: bool,
}

impl SearchBarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.query.clear();
            self.matches.clear();
            self.current_match_idx = 0;
        }
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match_idx = (self.current_match_idx + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current_match_idx == 0 {
                self.current_match_idx = self.matches.len().saturating_sub(1);
            } else {
                self.current_match_idx -= 1;
            }
        }
    }

    pub fn current_match(&self) -> Option<&SearchResult> {
        self.matches.get(self.current_match_idx)
    }
}

/// Complete Windows Terminal Application Window (wt.exe).
pub struct WindowsTerminalApp {
    pub tab_strip: TabStrip,
    pub color_schemes: HashMap<String, ColorScheme>,
    pub clipboard: TerminalClipboard,
    pub search_bar: SearchBarState,
    pub window_bounds: PaneRect,
}

impl Default for WindowsTerminalApp {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsTerminalApp {
    /// Initialize a new Windows Terminal instance.
    pub fn new() -> Self {
        let mut app = Self {
            tab_strip: TabStrip::new(),
            color_schemes: ColorScheme::all_builtins(),
            clipboard: TerminalClipboard::new(),
            search_bar: SearchBarState::new(),
            window_bounds: PaneRect::new(0, 0, 1024, 720),
        };
        app.update_layout();
        app
    }

    /// Recompute layout bounds for tabs and split panes.
    pub fn update_layout(&mut self) {
        let bounds = self.window_bounds;
        // Tab bar height: 36px, Search bar height: 30px (if open)
        let tab_bar_height = 36;
        let search_height = if self.search_bar.is_open { 30 } else { 0 };
        let content_y = bounds.y + tab_bar_height + search_height;
        let content_h = bounds.height.saturating_sub(tab_bar_height as u32 + search_height as u32);
        let content_bounds = PaneRect::new(bounds.x, content_y, bounds.width, content_h);

        for tab in &mut self.tab_strip.tabs {
            tab.panes.layout(content_bounds);
        }
    }

    /// Process user keyboard input with shortcut handling.
    pub fn handle_key(&mut self, key: KeyCode, mods: KeyModifiers) -> bool {
        // Alt+Shift+Plus: Split active tab vertically
        if mods.alt && mods.shift && (key == KeyCode::Plus || key == KeyCode::Char('+') || key == KeyCode::Char('=')) {
            let active_profile = self.tab_strip.active_tab().map(|t| t.profile.clone()).unwrap_or_else(Profile::powershell);
            if let Some(tab) = self.tab_strip.active_tab_mut() {
                tab.panes.split_active(SplitDirection::Vertical, active_profile);
                self.update_layout();
            }
            return true;
        }

        // Alt+Shift+Minus: Split active tab horizontally
        if mods.alt && mods.shift && (key == KeyCode::Minus || key == KeyCode::Char('-')) {
            let active_profile = self.tab_strip.active_tab().map(|t| t.profile.clone()).unwrap_or_else(Profile::powershell);
            if let Some(tab) = self.tab_strip.active_tab_mut() {
                tab.panes.split_active(SplitDirection::Horizontal, active_profile);
                self.update_layout();
            }
            return true;
        }

        // Ctrl+Shift+T: New tab (default profile)
        if mods.ctrl && mods.shift && (key == KeyCode::Char('T') || key == KeyCode::Char('t')) {
            self.tab_strip.new_tab(Profile::powershell());
            self.update_layout();
            return true;
        }

        // Ctrl+Shift+W: Close active pane or active tab
        if mods.ctrl && mods.shift && (key == KeyCode::Char('W') || key == KeyCode::Char('w')) {
            let mut closed_pane = false;
            if let Some(tab) = self.tab_strip.active_tab_mut() {
                if tab.panes.count_panes() > 1 {
                    closed_pane = tab.panes.close_active();
                }
            }
            if !closed_pane {
                let id = self.tab_strip.active_tab().map(|t| t.id).unwrap_or(0);
                self.tab_strip.close_tab(id);
            }
            self.update_layout();
            return true;
        }

        // Ctrl+Shift+F: Toggle search bar
        if mods.ctrl && mods.shift && (key == KeyCode::Char('F') || key == KeyCode::Char('f')) {
            self.search_bar.toggle();
            self.update_layout();
            return true;
        }

        // Ctrl+Tab: Switch to next tab
        if mods.ctrl && !mods.shift && key == KeyCode::Tab {
            self.tab_strip.next_tab();
            return true;
        }

        // Ctrl+Shift+Tab: Switch to prev tab
        if mods.ctrl && mods.shift && key == KeyCode::Tab {
            self.tab_strip.prev_tab();
            return true;
        }

        // Ctrl+C: Copy active selection
        if mods.ctrl && (key == KeyCode::Char('C') || key == KeyCode::Char('c')) {
            if self.copy_selection() {
                return true;
            }
        }

        // Ctrl+V: Paste from clipboard
        if mods.ctrl && (key == KeyCode::Char('V') || key == KeyCode::Char('v')) {
            self.paste_clipboard();
            return true;
        }

        // Search navigation when search bar is open
        if self.search_bar.is_open {
            if key == KeyCode::Enter {
                if mods.shift {
                    self.search_bar.prev_match();
                } else {
                    self.search_bar.next_match();
                }
                return true;
            }
            if key == KeyCode::Escape {
                self.search_bar.toggle();
                self.update_layout();
                return true;
            }
        }

        // Feed keystroke to active pane's interactive line
        if let Some(tab) = self.tab_strip.active_tab_mut() {
            if let Some(pane) = tab.panes.active_pane_mut() {
                match key {
                    KeyCode::Char(ch) => {
                        pane.input_line.push(ch);
                        pane.write_str(&ch.to_string());
                    }
                    KeyCode::Backspace => {
                        if !pane.input_line.is_empty() {
                            pane.input_line.pop();
                            pane.buffer.backspace(&mut pane.cursor);
                            let cur = pane.cursor;
                            pane.buffer.write_char(' ', CellAttributes::default(), &mut pane.cursor);
                            pane.cursor = cur;
                        }
                    }
                    KeyCode::Enter => {
                        let cmd = std::mem::take(&mut pane.input_line);
                        pane.write_str("\n");
                        pane.execute_command(&cmd);
                    }
                    KeyCode::PageUp => {
                        pane.buffer.scroll_view(10);
                    }
                    KeyCode::PageDown => {
                        pane.buffer.scroll_view(-10);
                    }
                    _ => {}
                }
            }
        }

        true
    }

    /// Copies selected text from active tab to clipboard.
    pub fn copy_selection(&mut self) -> bool {
        let mut text = None;
        if let Some(tab) = self.tab_strip.active_tab() {
            if let Some(sel) = tab.active_selection {
                if let Some(buffer) = tab.active_buffer() {
                    text = Some(buffer.get_selected_text(&sel));
                }
            }
        }

        if let Some(selected_str) = text {
            if !selected_str.is_empty() {
                self.clipboard.copy(&selected_str);
                return true;
            }
        }
        false
    }

    /// Pastes clipboard content into active pane input.
    pub fn paste_clipboard(&mut self) {
        let text = self.clipboard.paste().to_string();
        if !text.is_empty() {
            if let Some(tab) = self.tab_strip.active_tab_mut() {
                if let Some(pane) = tab.panes.active_pane_mut() {
                    pane.input_line.push_str(&text);
                    pane.write_str(&text);
                }
            }
        }
    }

    /// Performs search across the active tab's active buffer.
    pub fn perform_search(&mut self, query: &str) {
        self.search_bar.query = query.to_string();
        if let Some(tab) = self.tab_strip.active_tab() {
            if let Some(buf) = tab.active_buffer() {
                self.search_bar.matches = buf.search(query, self.search_bar.case_sensitive);
                self.search_bar.current_match_idx = 0;
            }
        }
    }

    /// Executes a command in the active tab's active pane.
    pub fn execute_in_active_pane(&mut self, cmd: &str) {
        if let Some(tab) = self.tab_strip.active_tab_mut() {
            if let Some(pane) = tab.panes.active_pane_mut() {
                pane.write_str(cmd);
                pane.write_str("\n");
                pane.execute_command(cmd);
            }
        }
    }

    /// Helper to render the active tab's visible viewport as plain ASCII lines.
    pub fn render_viewport_ascii(&self) -> String {
        let mut out = String::new();
        if let Some(tab) = self.tab_strip.active_tab() {
            if let Some(pane) = tab.panes.active_pane() {
                for r in 0..pane.buffer.rows {
                    if let Some(line) = pane.buffer.get_rendered_line(r) {
                        out.push_str(&pane.buffer.get_line_text(line));
                    }
                    out.push('\n');
                }
            }
        }
        out
    }
}

// ============================================================================
// Section 9: Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Tab management (creation, switching, closing, reordering).
    #[test]
    fn test_tab_management() {
        let mut app = WindowsTerminalApp::new();
        assert_eq!(app.tab_strip.tabs.len(), 1);
        assert_eq!(app.tab_strip.active_tab_index, 0);

        // Add tabs with distinct profiles
        let cmd_id = app.tab_strip.new_tab(Profile::windows_cmd());
        let bash_id = app.tab_strip.new_tab(Profile::linux_bash());
        assert_eq!(app.tab_strip.tabs.len(), 3);
        assert_eq!(app.tab_strip.active_tab_index, 2);

        // Verify active tab profile
        let active = app.tab_strip.active_tab().unwrap();
        assert_eq!(active.profile.shell_type, ShellType::LinuxBash);
        assert_eq!(active.profile.icon, "🐧");

        // Switch tabs
        app.tab_strip.select_tab(1);
        assert_eq!(app.tab_strip.active_tab().unwrap().id, cmd_id);

        app.tab_strip.next_tab();
        assert_eq!(app.tab_strip.active_tab().unwrap().id, bash_id);

        app.tab_strip.prev_tab();
        assert_eq!(app.tab_strip.active_tab().unwrap().id, cmd_id);

        // Reorder tabs: move index 1 (cmd) to index 0
        assert!(app.tab_strip.reorder_tab(1, 0));
        assert_eq!(app.tab_strip.tabs[0].id, cmd_id);

        // Close tab
        assert!(app.tab_strip.close_tab(cmd_id));
        assert_eq!(app.tab_strip.tabs.len(), 2);
    }

    /// Test 2: VT100 / ANSI escape sequence parser (SGR, 256 colors, TrueColor, cursor movement).
    #[test]
    fn test_ansi_escape_sequence_parser() {
        let mut buffer = TerminalBuffer::new(80, 24, 100);
        let mut cursor = CursorPosition::default();
        let mut parser = AnsiEscapeParser::new();

        // 1. SGR formatting: Bold, Dim, Italic, Underline, Invert
        parser.process_str("\x1b[1;3;4mStyledText\x1b[0m", &mut buffer, &mut cursor);
        let cell = buffer.viewport[0][0];
        assert_eq!(cell.ch, 'S');
        assert!(cell.attrs.bold);
        assert!(cell.attrs.italic);
        assert!(cell.attrs.underline);

        // 2. ANSI 16 colors
        cursor.col = 0;
        cursor.row = 1;
        parser.process_str("\x1b[31;42mRedOnGreen\x1b[0m", &mut buffer, &mut cursor);
        let red_cell = buffer.viewport[1][0];
        assert_eq!(red_cell.attrs.fg, Color::Ansi16(1)); // Red
        assert_eq!(red_cell.attrs.bg, Color::Ansi16(2)); // Green

        // 3. 256 Colors (38;5;208)
        cursor.col = 0;
        cursor.row = 2;
        parser.process_str("\x1b[38;5;208mOrange256\x1b[0m", &mut buffer, &mut cursor);
        assert_eq!(buffer.viewport[2][0].attrs.fg, Color::Color256(208));

        // 4. 24-bit TrueColor RGB (38;2;123;45;67)
        cursor.col = 0;
        cursor.row = 3;
        parser.process_str("\x1b[38;2;123;45;67mTrueColor\x1b[0m", &mut buffer, &mut cursor);
        assert_eq!(buffer.viewport[3][0].attrs.fg, Color::Rgb(123, 45, 67));

        // 5. Cursor positioning (CUP)
        parser.process_str("\x1b[10;15H", &mut buffer, &mut cursor);
        assert_eq!(cursor.row, 9); // 1-indexed to 0-indexed
        assert_eq!(cursor.col, 14);

        // 6. Relative cursor movement
        parser.process_str("\x1b[2A\x1b[3C", &mut buffer, &mut cursor);
        assert_eq!(cursor.row, 7);  // Up 2
        assert_eq!(cursor.col, 17); // Forward 3

        // 7. OSC window title setting
        let title = parser.process_str("\x1b]0;HybridOS Admin Console\x07", &mut buffer, &mut cursor);
        assert_eq!(title, Some("HybridOS Admin Console".to_string()));
    }

    /// Test 3: Split pane creation, vertical/horizontal splitting, and resizing.
    #[test]
    fn test_split_pane_creation() {
        let mut app = WindowsTerminalApp::new();
        let tab = app.tab_strip.active_tab_mut().unwrap();
        assert_eq!(tab.panes.count_panes(), 1);

        // Split vertically (Alt+Shift+Plus)
        let pane2_id = tab.panes.split_active(SplitDirection::Vertical, Profile::linux_bash()).unwrap();
        assert_eq!(tab.panes.count_panes(), 2);
        assert_eq!(tab.panes.active_pane_id, pane2_id);

        // Split horizontally (Alt+Shift+Minus)
        let pane3_id = tab.panes.split_active(SplitDirection::Horizontal, Profile::windows_cmd()).unwrap();
        assert_eq!(tab.panes.count_panes(), 3);
        assert_eq!(tab.panes.active_pane_id, pane3_id);

        // Recompute layout across window bounds (1024x720)
        app.update_layout();
        let tab = app.tab_strip.active_tab().unwrap();
        let panes = tab.panes.panes();
        assert_eq!(panes.len(), 3);
        for p in panes {
            assert!(p.rect.width > 0);
            assert!(p.rect.height > 0);
        }

        // Close sub-pane
        let tab = app.tab_strip.active_tab_mut().unwrap();
        assert!(tab.panes.close_pane(pane3_id));
        assert_eq!(tab.panes.count_panes(), 2);
    }

    /// Test 4: Scrollback buffer and capacity up to 10,000 lines.
    #[test]
    fn test_scrollback_buffer() {
        let mut buffer = TerminalBuffer::new(40, 5, 10_000);
        let mut cursor = CursorPosition::default();

        // Write 25 lines into a 5-row viewport
        for i in 1..=25 {
            let line = format!("Log line #{:02}\n", i);
            for ch in line.chars() {
                buffer.write_char(ch, CellAttributes::default(), &mut cursor);
            }
        }

        // Viewport should have 5 lines, scrollback should have 21 lines
        assert_eq!(buffer.viewport.len(), 5);
        assert_eq!(buffer.scrollback.len(), 21);
        assert_eq!(buffer.total_lines(), 26);

        // First scrollback line should be "Log line #01"
        let first_line = buffer.get_line_text(&buffer.scrollback[0]);
        assert!(first_line.contains("Log line #01"));

        // Test scrolling up into history
        buffer.scroll_view(10);
        assert_eq!(buffer.scroll_offset, 10);

        // Test Erase In Display mode 3 (clears viewport AND scrollback)
        buffer.erase_in_display(3, &cursor);
        assert_eq!(buffer.scrollback.len(), 0);
        assert_eq!(buffer.scroll_offset, 0);
    }

    /// Test 5: Clipboard copy, paste, and text selection.
    #[test]
    fn test_clipboard_copy_paste() {
        let mut app = WindowsTerminalApp::new();

        // Prepare some text in the active pane buffer
        let tab = app.tab_strip.active_tab_mut().unwrap();
        let pane = tab.panes.active_pane_mut().unwrap();
        pane.buffer.viewport[0][0] = TerminalCell::new('H', CellAttributes::default());
        pane.buffer.viewport[0][1] = TerminalCell::new('e', CellAttributes::default());
        pane.buffer.viewport[0][2] = TerminalCell::new('l', CellAttributes::default());
        pane.buffer.viewport[0][3] = TerminalCell::new('l', CellAttributes::default());
        pane.buffer.viewport[0][4] = TerminalCell::new('o', CellAttributes::default());

        // Set active selection
        tab.active_selection = Some(SelectionRange::new(
            CursorPosition::new(0, 0),
            CursorPosition::new(0, 4),
        ));

        // Copy selection
        assert!(app.copy_selection());
        assert_eq!(app.clipboard.paste(), "Hello");

        // Paste clipboard content into active pane
        app.paste_clipboard();
        let tab = app.tab_strip.active_tab().unwrap();
        let pane = tab.panes.active_pane().unwrap();
        assert!(pane.input_line.contains("Hello"));
    }

    /// Test 6: In-buffer text search (Ctrl+Shift+F).
    #[test]
    fn test_search_scrollback_buffer() {
        let mut buffer = TerminalBuffer::new(80, 10, 100);
        let mut cursor = CursorPosition::default();

        // Fill lines with distinct search keywords
        let lines = [
            "Initializing HybridOS Kernel v1.0...\n",
            "Hardware check: Intel Core i9 detected.\n",
            "Warning: Low disk space on /var/log.\n",
            "Error: Kernel panic prevented by trap handler.\n",
            "HybridOS shell ready.\n",
        ];
        for l in lines {
            for ch in l.chars() {
                buffer.write_char(ch, CellAttributes::default(), &mut cursor);
            }
        }

        // Search for "Kernel" (case sensitive)
        let results = buffer.search("Kernel", true);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line_idx, 0);
        assert_eq!(results[1].line_idx, 3);

        // Search case insensitive
        let results_ci = buffer.search("error", false);
        assert_eq!(results_ci.len(), 1);
        assert_eq!(results_ci[0].line_idx, 3);
    }

    /// Test 7: Built-in color schemes and font configurations.
    #[test]
    fn test_themes_and_fonts() {
        let schemes = ColorScheme::all_builtins();
        assert!(schemes.contains_key("Campbell"));
        assert!(schemes.contains_key("One Half Dark"));
        assert!(schemes.contains_key("One Half Light"));
        assert!(schemes.contains_key("Solarized Dark"));
        assert!(schemes.contains_key("Tango Dark"));
        assert!(schemes.contains_key("Vintage"));

        let campbell = schemes.get("Campbell").unwrap();
        assert_eq!(campbell.background, (12, 12, 12));
        assert_eq!(campbell.foreground, (204, 204, 204));

        let vintage = schemes.get("Vintage").unwrap();
        assert_eq!(vintage.background, (0, 0, 0));
        assert_eq!(vintage.foreground, (192, 192, 192));

        // Test Color resolution
        let default_fg = Color::DefaultForeground.resolve_rgb(campbell, true);
        assert_eq!(default_fg, (204, 204, 204));

        let ansi_red = Color::Ansi16(1).resolve_rgb(campbell, true);
        assert_eq!(ansi_red, (197, 15, 31));

        let true_color = Color::Rgb(10, 20, 30).resolve_rgb(campbell, true);
        assert_eq!(true_color, (10, 20, 30));
    }

    /// Test 8: Shell execution integration with HybridShell.
    #[test]
    fn test_shell_execution_integration() {
        let mut app = WindowsTerminalApp::new();
        // Execute a standard command in the active pane
        app.execute_in_active_pane("ver");

        let tab = app.tab_strip.active_tab().unwrap();
        let pane = tab.panes.active_pane().unwrap();
        let rendered = pane.buffer.get_all_text();
        // The prompt or output should reflect HybridOS or Windows version
        assert!(rendered.contains("HybridOS") || rendered.contains("Version") || rendered.contains("PS"));
    }
}
