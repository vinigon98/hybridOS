//! Windows 11 Core Commercial Productivity Applications
//!
//! Production-grade implementation of Windows 11 built-in productivity tools for HybridOS:
//! 1. **Windows 11 Modern Notepad**:
//!    - Multi-tab document architecture with unique tab identifiers, file path tracking, dirty state,
//!      dynamic text buffer, and transactional undo/redo action stacks.
//!    - Windows 11 Fluent Menu bar: File (New Tab, New Window, Open, Save, Save As, Exit),
//!      Edit (Undo, Redo, Cut, Copy, Paste, Delete, Find, Replace, Select All, Time/Date),
//!      View (Zoom In/Out/Reset, Status Bar toggle, Word Wrap toggle).
//!    - Windows 11 Status bar: Line and column cursor indicator, zoom percentage (10% - 500%),
//!      smart Windows (CRLF) / Unix (LF) line ending detector and converter, UTF-8/UTF-16 encoding,
//!      character count and word count statistics.
//! 2. **Windows 11 Modern Calculator**:
//!    - Fluent mode switcher: Standard, Scientific, and Programmer.
//!    - Standard Mode: Full numeric pad (0-9, decimal), clear (C), clear entry (CE), backspace (⌫),
//!      basic operators (+, -, *, /, =), unary operators (%, √x, x², 1/x, +/-), and complete memory
//!      management (MC, MR, M+, M-, MS, Mv).
//!    - Programmer Mode: Real-time 4-base synchronization (HEX, DEC, OCT, BIN), bit-width word sizing
//!      (QWORD 64-bit, DWORD 32-bit, WORD 16-bit, BYTE 8-bit), complete bitwise operator engine
//!      (AND, OR, NOT, XOR, Lsh, Rsh, Ash, RoL, RoR), and 64-bit interactive bit toggle matrix.
//!    - Scientific Mode: Trigonometric functions (sin, cos, tan, asin, acos, atan in Deg/Rad/Grad),
//!      logarithms (ln, log10, log2), exponential powers, roots, factorial (n!), and parentheses.
//!    - Active equation preview (e.g. "125 + 45 =") and persistent calculation history roll.
//! 3. **Windows 11 Snipping Tool**:
//!    - 4 Commercial capture modes: Rectangle Snip, Freeform Snip (ray-cast polygon mask),
//!      Window Snip, and Fullscreen Snip.
//!    - Delay timer state machine: No delay (0s), 3 seconds, 5 seconds, 10 seconds.
//!    - Annotation Canvas: Ballpoint Pen (color, thickness 1-24px), Highlighter (semi-transparent
//!      alpha-blended tint), Stroke/Point Eraser, Straight Edge Ruler, and 360° Protractor guide.
//!    - Actions & Export: Instant clipboard copy, pure Rust zero-dependency PNG encoder (valid IHDR,
//!      IDAT uncompressed/deflate, IEND chunks with Adler32 and CRC32), 32-bit BMP encoder, and Share.
//! 4. **100% Pure Safe Rust**: Zero unsafe code, robust bounds checking, comprehensive unit tests.

#![forbid(unsafe_code)]
#![allow(dead_code)]

// ============================================================================
// Section 1: Geometry, Color & Fluent Drawing Primitives
// ============================================================================

/// 2D coordinate point
#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_squared(&self, other: Point) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        dx * dx + dy * dy
    }

    pub fn distance(&self, other: Point) -> f64 {
        (self.distance_squared(other) as f64).sqrt()
    }
}

/// 2D Size dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const ZERO: Self = Self { width: 0, height: 0 };

    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }
}

/// 2D Axis-aligned Bounding Box
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const EMPTY: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn from_points(p1: Point, p2: Point) -> Self {
        let x = p1.x.min(p2.x);
        let y = p1.y.min(p2.y);
        let width = (p1.x.max(p2.x) - x) as u32;
        let height = (p1.y.max(p2.y) - y) as u32;
        Self { x, y, width, height }
    }

    pub fn left(&self) -> i32 {
        self.x
    }

    pub fn top(&self) -> i32 {
        self.y
    }

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.left() < other.right() &&
        self.right() > other.left() &&
        self.top() < other.bottom() &&
        self.bottom() > other.top()
    }

    pub fn clamp_to(&self, bounds: Rect) -> Rect {
        let x1 = self.left().max(bounds.left());
        let y1 = self.top().max(bounds.top());
        let x2 = self.right().min(bounds.right());
        let y2 = self.bottom().min(bounds.bottom());

        if x2 > x1 && y2 > y1 {
            Rect {
                x: x1,
                y: y1,
                width: (x2 - x1) as u32,
                height: (y2 - y1) as u32,
            }
        } else {
            Rect::EMPTY
        }
    }
}

/// 32-bit RGBA Color with standard Fluent Design swatches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const RED: Self = Self { r: 232, g: 17, b: 35, a: 255 };
    pub const GREEN: Self = Self { r: 16, g: 124, b: 65, a: 255 };
    pub const BLUE: Self = Self { r: 0, g: 120, b: 215, a: 255 };
    pub const YELLOW: Self = Self { r: 255, g: 216, b: 0, a: 255 };
    pub const HIGHLIGHT_YELLOW: Self = Self { r: 255, g: 255, b: 0, a: 128 };
    pub const HIGHLIGHT_GREEN: Self = Self { r: 0, g: 255, b: 128, a: 128 };
    pub const HIGHLIGHT_CYAN: Self = Self { r: 0, g: 220, b: 255, a: 128 };
    pub const HIGHLIGHT_PINK: Self = Self { r: 255, g: 105, b: 180, a: 128 };
    pub const WIN11_ACCENT: Self = Self { r: 0, g: 103, b: 192, a: 255 };
    pub const SURFACE_DARK: Self = Self { r: 32, g: 32, b: 32, a: 255 };
    pub const SURFACE_LIGHT: Self = Self { r: 243, g: 243, b: 243, a: 255 };

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_u32_argb(val: u32) -> Self {
        Self {
            a: ((val >> 24) & 0xFF) as u8,
            r: ((val >> 16) & 0xFF) as u8,
            g: ((val >> 8) & 0xFF) as u8,
            b: (val & 0xFF) as u8,
        }
    }

    pub const fn to_u32_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub const fn to_u32_rgba(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }

    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self { r: self.r, g: self.g, b: self.b, a: alpha }
    }

    /// Alpha composite `self` over `backdrop` (standard Porter-Duff source-over)
    pub fn blend_over(&self, backdrop: Color) -> Color {
        if self.a == 255 {
            return *self;
        }
        if self.a == 0 {
            return backdrop;
        }

        let sa = self.a as u32;
        let da = backdrop.a as u32;
        let inv_sa = 255 - sa;

        let out_a = sa + (da * inv_sa) / 255;
        if out_a == 0 {
            return Color::TRANSPARENT;
        }

        let out_r = ((self.r as u32 * sa + backdrop.r as u32 * da * inv_sa / 255) / out_a).min(255) as u8;
        let out_g = ((self.g as u32 * sa + backdrop.g as u32 * da * inv_sa / 255) / out_a).min(255) as u8;
        let out_b = ((self.b as u32 * sa + backdrop.b as u32 * da * inv_sa / 255) / out_a).min(255) as u8;

        Color::rgba(out_r, out_g, out_b, out_a as u8)
    }
}

// ============================================================================
// Section 2: Windows 11 Modern Notepad
// ============================================================================

/// Line ending convention
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    WindowsCrlf,
    UnixLf,
    MacintoshCr,
}

impl LineEnding {
    pub fn display_name(&self) -> &'static str {
        match self {
            LineEnding::WindowsCrlf => "Windows (CRLF)",
            LineEnding::UnixLf => "Unix (LF)",
            LineEnding::MacintoshCr => "Macintosh (CR)",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::WindowsCrlf => "\r\n",
            LineEnding::UnixLf => "\n",
            LineEnding::MacintoshCr => "\r",
        }
    }
}

/// Character encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Ansi,
}

impl TextEncoding {
    pub fn display_name(&self) -> &'static str {
        match self {
            TextEncoding::Utf8 => "UTF-8",
            TextEncoding::Utf16Le => "UTF-16 LE",
            TextEncoding::Utf16Be => "UTF-16 BE",
            TextEncoding::Ansi => "ANSI",
        }
    }
}

/// Text action recorded in undo / redo history
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextUndoAction {
    Insert {
        byte_offset: usize,
        text: String,
    },
    Delete {
        byte_offset: usize,
        text: String,
    },
    Replace {
        byte_offset: usize,
        old_text: String,
        new_text: String,
    },
}

/// Search direction for Find & Replace
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// Find and Replace configuration options
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindReplaceOptions {
    pub query: String,
    pub replacement: String,
    pub match_case: bool,
    pub match_whole_word: bool,
    pub wrap_around: bool,
    pub direction: SearchDirection,
}

impl Default for FindReplaceOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            match_case: false,
            match_whole_word: false,
            wrap_around: true,
            direction: SearchDirection::Forward,
        }
    }
}

/// Match location in text buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindResult {
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub col: usize,
}

/// An open document tab inside Windows 11 Modern Notepad
#[derive(Debug, Clone)]
pub struct NotepadTab {
    pub id: u64,
    pub title: String,
    pub file_path: Option<String>,
    pub is_dirty: bool,
    pub text: String,
    pub cursor_byte_offset: usize,
    pub selection_start: Option<usize>,
    pub selection_end: Option<usize>,
    pub undo_stack: Vec<TextUndoAction>,
    pub redo_stack: Vec<TextUndoAction>,
    pub line_ending: LineEnding,
    pub encoding: TextEncoding,
    pub zoom_level: u32,
    pub word_wrap: bool,
    pub status_bar_visible: bool,
}

impl NotepadTab {
    pub fn new(id: u64, title: String) -> Self {
        Self {
            id,
            title,
            file_path: None,
            is_dirty: false,
            text: String::new(),
            cursor_byte_offset: 0,
            selection_start: None,
            selection_end: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            line_ending: LineEnding::WindowsCrlf,
            encoding: TextEncoding::Utf8,
            zoom_level: 100,
            word_wrap: false,
            status_bar_visible: true,
        }
    }

    pub fn with_content(id: u64, title: String, file_path: Option<String>, content: String) -> Self {
        let line_ending = Self::detect_line_ending(&content);
        let mut tab = Self::new(id, title);
        tab.file_path = file_path;
        tab.text = content;
        tab.line_ending = line_ending;
        tab.cursor_byte_offset = 0;
        tab.is_dirty = false;
        tab
    }

    /// Auto-detect CRLF vs LF line endings
    pub fn detect_line_ending(text: &str) -> LineEnding {
        let mut crlf_count = 0usize;
        let mut lf_count = 0usize;
        let bytes = text.as_bytes();

        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\r' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf_count += 1;
                    i += 2;
                    continue;
                }
            } else if bytes[i] == b'\n' {
                lf_count += 1;
            }
            i += 1;
        }

        if crlf_count >= lf_count && crlf_count > 0 {
            LineEnding::WindowsCrlf
        } else if lf_count > 0 {
            LineEnding::UnixLf
        } else {
            LineEnding::WindowsCrlf
        }
    }

    /// Convert all line breaks to target LineEnding
    pub fn convert_line_endings(&mut self, target: LineEnding) {
        if self.line_ending == target {
            return;
        }

        let old_text = self.text.clone();
        let normalized = self.text.replace("\r\n", "\n").replace('\r', "\n");
        let new_text = match target {
            LineEnding::WindowsCrlf => normalized.replace('\n', "\r\n"),
            LineEnding::UnixLf => normalized,
            LineEnding::MacintoshCr => normalized.replace('\n', "\r"),
        };

        if old_text != new_text {
            self.undo_stack.push(TextUndoAction::Replace {
                byte_offset: 0,
                old_text,
                new_text: new_text.clone(),
            });
            self.redo_stack.clear();
            self.text = new_text;
            self.line_ending = target;
            self.is_dirty = true;
            self.cursor_byte_offset = self.cursor_byte_offset.min(self.text.len());
        } else {
            self.line_ending = target;
        }
    }

    /// Insert text at current cursor location, clearing active selection if any
    pub fn insert_str(&mut self, input: &str) {
        if input.is_empty() {
            return;
        }

        self.delete_selection();

        let pos = self.cursor_byte_offset.min(self.text.len());
        self.text.insert_str(pos, input);

        self.undo_stack.push(TextUndoAction::Insert {
            byte_offset: pos,
            text: input.to_string(),
        });
        self.redo_stack.clear();

        self.cursor_byte_offset = pos + input.len();
        self.is_dirty = true;
    }

    /// Insert single character
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_str(s);
    }

    /// Backspace key: deletes character before cursor or deletes selection
    pub fn delete_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }

        if self.cursor_byte_offset == 0 || self.text.is_empty() {
            return false;
        }

        let pos = self.cursor_byte_offset.min(self.text.len());
        // Find previous UTF-8 boundary
        let mut prev_pos = pos - 1;
        while prev_pos > 0 && !self.text.is_char_boundary(prev_pos) {
            prev_pos -= 1;
        }

        let deleted_text = self.text[prev_pos..pos].to_string();
        self.text.drain(prev_pos..pos);

        self.undo_stack.push(TextUndoAction::Delete {
            byte_offset: prev_pos,
            text: deleted_text,
        });
        self.redo_stack.clear();

        self.cursor_byte_offset = prev_pos;
        self.is_dirty = true;
        true
    }

    /// Delete key: deletes character after cursor or deletes selection
    pub fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }

        let pos = self.cursor_byte_offset;
        if pos >= self.text.len() {
            return false;
        }

        // Find next UTF-8 boundary
        let mut next_pos = pos + 1;
        while next_pos < self.text.len() && !self.text.is_char_boundary(next_pos) {
            next_pos += 1;
        }

        let deleted_text = self.text[pos..next_pos].to_string();
        self.text.drain(pos..next_pos);

        self.undo_stack.push(TextUndoAction::Delete {
            byte_offset: pos,
            text: deleted_text,
        });
        self.redo_stack.clear();

        self.is_dirty = true;
        true
    }

    /// Delete selected text range
    pub fn delete_selection(&mut self) -> bool {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (p1, p2) = if start <= end { (start, end) } else { (end, start) };
            let p1 = p1.min(self.text.len());
            let p2 = p2.min(self.text.len());

            if p1 != p2 {
                let deleted = self.text[p1..p2].to_string();
                self.text.drain(p1..p2);

                self.undo_stack.push(TextUndoAction::Delete {
                    byte_offset: p1,
                    text: deleted,
                });
                self.redo_stack.clear();

                self.cursor_byte_offset = p1;
                self.selection_start = None;
                self.selection_end = None;
                self.is_dirty = true;
                return true;
            }
        }
        self.selection_start = None;
        self.selection_end = None;
        false
    }

    /// Undo last edit action
    pub fn undo(&mut self) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            match action {
                TextUndoAction::Insert { byte_offset, ref text } => {
                    let end_pos = (byte_offset + text.len()).min(self.text.len());
                    let removed = self.text[byte_offset..end_pos].to_string();
                    self.text.drain(byte_offset..end_pos);
                    self.cursor_byte_offset = byte_offset;
                    self.redo_stack.push(TextUndoAction::Insert {
                        byte_offset,
                        text: removed,
                    });
                }
                TextUndoAction::Delete { byte_offset, ref text } => {
                    let pos = byte_offset.min(self.text.len());
                    self.text.insert_str(pos, text);
                    self.cursor_byte_offset = pos + text.len();
                    self.redo_stack.push(TextUndoAction::Delete {
                        byte_offset,
                        text: text.clone(),
                    });
                }
                TextUndoAction::Replace { byte_offset, ref old_text, ref new_text } => {
                    let end_pos = (byte_offset + new_text.len()).min(self.text.len());
                    self.text.drain(byte_offset..end_pos);
                    self.text.insert_str(byte_offset, old_text);
                    self.cursor_byte_offset = byte_offset + old_text.len();
                    self.redo_stack.push(TextUndoAction::Replace {
                        byte_offset,
                        old_text: old_text.clone(),
                        new_text: new_text.clone(),
                    });
                }
            }
            self.selection_start = None;
            self.selection_end = None;
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    /// Redo previously undone action
    pub fn redo(&mut self) -> bool {
        if let Some(action) = self.redo_stack.pop() {
            match action {
                TextUndoAction::Insert { byte_offset, ref text } => {
                    let pos = byte_offset.min(self.text.len());
                    self.text.insert_str(pos, text);
                    self.cursor_byte_offset = pos + text.len();
                    self.undo_stack.push(TextUndoAction::Insert {
                        byte_offset,
                        text: text.clone(),
                    });
                }
                TextUndoAction::Delete { byte_offset, ref text } => {
                    let end_pos = (byte_offset + text.len()).min(self.text.len());
                    let removed = self.text[byte_offset..end_pos].to_string();
                    self.text.drain(byte_offset..end_pos);
                    self.cursor_byte_offset = byte_offset;
                    self.undo_stack.push(TextUndoAction::Delete {
                        byte_offset,
                        text: removed,
                    });
                }
                TextUndoAction::Replace { byte_offset, ref old_text, ref new_text } => {
                    let end_pos = (byte_offset + old_text.len()).min(self.text.len());
                    self.text.drain(byte_offset..end_pos);
                    self.text.insert_str(byte_offset, new_text);
                    self.cursor_byte_offset = byte_offset + new_text.len();
                    self.undo_stack.push(TextUndoAction::Replace {
                        byte_offset,
                        old_text: old_text.clone(),
                        new_text: new_text.clone(),
                    });
                }
            }
            self.selection_start = None;
            self.selection_end = None;
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    /// Select entire text buffer (Ctrl+A)
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_start = Some(0);
            self.selection_end = Some(self.text.len());
            self.cursor_byte_offset = self.text.len();
        }
    }

    /// Get current selection substring
    pub fn selected_text(&self) -> Option<&str> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (p1, p2) = if start <= end { (start, end) } else { (end, start) };
            if p1 < self.text.len() && p2 <= self.text.len() && p1 != p2 {
                return Some(&self.text[p1..p2]);
            }
        }
        None
    }

    /// Calculate 1-indexed Line and Column position from current byte offset
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let offset = self.cursor_byte_offset.min(self.text.len());
        let slice = &self.text[..offset];

        let mut line = 1;
        let mut last_line_start = 0;

        for (i, c) in slice.char_indices() {
            if c == '\n' {
                line += 1;
                last_line_start = i + 1;
            }
        }

        let col = slice[last_line_start..].chars().count() + 1;
        (line, col)
    }

    /// Set cursor by 1-indexed line and column
    pub fn set_cursor_line_col(&mut self, target_line: usize, target_col: usize) {
        let mut current_line = 1;
        let mut byte_idx = 0;

        for line_str in self.text.split_inclusive('\n') {
            if current_line == target_line {
                let mut col_count = 1;
                for (char_idx, _) in line_str.char_indices() {
                    if col_count == target_col {
                        self.cursor_byte_offset = byte_idx + char_idx;
                        return;
                    }
                    col_count += 1;
                }
                self.cursor_byte_offset = byte_idx + line_str.trim_end_matches(&['\r', '\n'][..]).len();
                return;
            }
            byte_idx += line_str.len();
            current_line += 1;
        }

        self.cursor_byte_offset = self.text.len();
    }

    /// Zoom In (up to 500%)
    pub fn zoom_in(&mut self) {
        if self.zoom_level < 500 {
            self.zoom_level = (self.zoom_level + 10).min(500);
        }
    }

    /// Zoom Out (down to 10%)
    pub fn zoom_out(&mut self) {
        if self.zoom_level > 10 {
            self.zoom_level = (self.zoom_level - 10).max(10);
        }
    }

    /// Reset Zoom to 100%
    pub fn reset_zoom(&mut self) {
        self.zoom_level = 100;
    }

    /// Document statistics: (characters, words, lines)
    pub fn statistics(&self) -> (usize, usize, usize) {
        let chars = self.text.chars().count();
        let words = self.text.split_whitespace().count();
        let lines = if self.text.is_empty() {
            1
        } else {
            self.text.split('\n').count()
        };
        (chars, words, lines)
    }

    /// Find next occurrence of query matching options
    pub fn find(&self, opts: &FindReplaceOptions, from_pos: usize) -> Option<FindResult> {
        if opts.query.is_empty() || self.text.is_empty() {
            return None;
        }

        let query = if opts.match_case {
            opts.query.clone()
        } else {
            opts.query.to_lowercase()
        };

        let target_text = if opts.match_case {
            self.text.clone()
        } else {
            self.text.to_lowercase()
        };

        let find_in_range = |haystack: &str, start_offset: usize| -> Option<usize> {
            let mut search_idx = 0;
            while let Some(found_rel) = haystack[search_idx..].find(&query) {
                let absolute = search_idx + found_rel;
                if opts.match_whole_word {
                    let before_ok = absolute == 0 || !haystack[..absolute].chars().last().unwrap().is_alphanumeric();
                    let after_pos = absolute + query.len();
                    let after_ok = after_pos >= haystack.len() || !haystack[after_pos..].chars().next().unwrap().is_alphanumeric();
                    if before_ok && after_ok {
                        return Some(start_offset + absolute);
                    }
                } else {
                    return Some(start_offset + absolute);
                }
                search_idx = absolute + 1;
                while search_idx < haystack.len() && !haystack.is_char_boundary(search_idx) {
                    search_idx += 1;
                }
            }
            None
        };

        let safe_from = from_pos.min(self.text.len());
        // Forward search from cursor
        if let Some(pos) = find_in_range(&target_text[safe_from..], safe_from) {
            let (line, col) = self.calculate_line_col_at(pos);
            return Some(FindResult {
                start_byte: pos,
                end_byte: pos + opts.query.len(),
                line,
                col,
            });
        }

        // Wrap around search
        if opts.wrap_around && safe_from > 0 {
            if let Some(pos) = find_in_range(&target_text[..safe_from], 0) {
                let (line, col) = self.calculate_line_col_at(pos);
                return Some(FindResult {
                    start_byte: pos,
                    end_byte: pos + opts.query.len(),
                    line,
                    col,
                });
            }
        }

        None
    }

    /// Replace single match
    pub fn replace(&mut self, opts: &FindReplaceOptions) -> bool {
        if let Some(found) = self.find(opts, self.cursor_byte_offset) {
            let old_text = self.text[found.start_byte..found.end_byte].to_string();
            self.text.drain(found.start_byte..found.end_byte);
            self.text.insert_str(found.start_byte, &opts.replacement);

            self.undo_stack.push(TextUndoAction::Replace {
                byte_offset: found.start_byte,
                old_text,
                new_text: opts.replacement.clone(),
            });
            self.redo_stack.clear();

            self.cursor_byte_offset = found.start_byte + opts.replacement.len();
            self.selection_start = Some(found.start_byte);
            self.selection_end = Some(self.cursor_byte_offset);
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    /// Replace all occurrences
    pub fn replace_all(&mut self, opts: &FindReplaceOptions) -> usize {
        let mut count = 0;
        let mut search_from = 0;

        let mut no_wrap_opts = opts.clone();
        no_wrap_opts.wrap_around = false;

        while let Some(found) = self.find(&no_wrap_opts, search_from) {
            let old_text = self.text[found.start_byte..found.end_byte].to_string();
            self.text.drain(found.start_byte..found.end_byte);
            self.text.insert_str(found.start_byte, &opts.replacement);

            self.undo_stack.push(TextUndoAction::Replace {
                byte_offset: found.start_byte,
                old_text,
                new_text: opts.replacement.clone(),
            });

            count += 1;
            search_from = found.start_byte + opts.replacement.len();
            if search_from >= self.text.len() {
                break;
            }
        }

        if count > 0 {
            self.redo_stack.clear();
            self.is_dirty = true;
        }
        count
    }

    fn calculate_line_col_at(&self, byte_offset: usize) -> (usize, usize) {
        let slice = &self.text[..byte_offset.min(self.text.len())];
        let mut line = 1;
        let mut last_line_start = 0;
        for (i, c) in slice.char_indices() {
            if c == '\n' {
                line += 1;
                last_line_start = i + 1;
            }
        }
        let col = slice[last_line_start..].chars().count() + 1;
        (line, col)
    }
}

/// Windows 11 Modern Notepad Application Instance
#[derive(Debug, Clone)]
pub struct ModernNotepad {
    pub tabs: Vec<NotepadTab>,
    pub active_tab_index: usize,
    pub next_tab_id: u64,
    pub clipboard: String,
    pub find_replace_dialog: FindReplaceOptions,
}

impl ModernNotepad {
    pub fn new() -> Self {
        let mut app = Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 1,
            clipboard: String::new(),
            find_replace_dialog: FindReplaceOptions::default(),
        };
        app.new_tab();
        app
    }

    /// Create and switch to a new Untitled tab
    pub fn new_tab(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let title = format!("Untitled {}", id);
        let tab = NotepadTab::new(id, title);
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        id
    }

    /// Open an existing file into a new tab
    pub fn open_file(&mut self, file_path: &str, content: &str) -> u64 {
        // If the only tab is Untitled, empty, and clean, reuse it
        if self.tabs.len() == 1 && !self.tabs[0].is_dirty && self.tabs[0].text.is_empty() {
            let id = self.tabs[0].id;
            let filename = std::path::Path::new(file_path)
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Document".to_string());
            self.tabs[0].title = filename;
            self.tabs[0].file_path = Some(file_path.to_string());
            self.tabs[0].text = content.to_string();
            self.tabs[0].line_ending = NotepadTab::detect_line_ending(content);
            self.tabs[0].cursor_byte_offset = 0;
            self.tabs[0].is_dirty = false;
            return id;
        }

        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let filename = std::path::Path::new(file_path)
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Document".to_string());
        let tab = NotepadTab::with_content(id, filename, Some(file_path.to_string()), content.to_string());
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        id
    }

    /// Close tab by id
    pub fn close_tab(&mut self, tab_id: u64) -> Result<(), &'static str> {
        if self.tabs.len() <= 1 {
            // Keep at least one tab open
            self.tabs[0] = NotepadTab::new(self.next_tab_id, format!("Untitled {}", self.next_tab_id));
            self.next_tab_id += 1;
            self.active_tab_index = 0;
            return Ok(());
        }

        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(pos);
            if self.active_tab_index >= self.tabs.len() {
                self.active_tab_index = self.tabs.len().saturating_sub(1);
            }
            Ok(())
        } else {
            Err("Tab not found")
        }
    }

    /// Switch active tab by ID
    pub fn switch_tab(&mut self, tab_id: u64) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.active_tab_index = pos;
            true
        } else {
            false
        }
    }

    /// Reorder tabs (drag and drop)
    pub fn reorder_tabs(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index < self.tabs.len() && to_index < self.tabs.len() {
            let tab = self.tabs.remove(from_index);
            self.tabs.insert(to_index, tab);
            self.active_tab_index = to_index;
            true
        } else {
            false
        }
    }

    /// Reference to currently active tab
    pub fn active_tab(&self) -> &NotepadTab {
        &self.tabs[self.active_tab_index]
    }

    /// Mutable reference to currently active tab
    pub fn active_tab_mut(&mut self) -> &mut NotepadTab {
        &mut self.tabs[self.active_tab_index]
    }

    /// Clipboard Cut
    pub fn cut(&mut self) -> bool {
        let tab = &mut self.tabs[self.active_tab_index];
        if let Some(sel) = tab.selected_text() {
            self.clipboard = sel.to_string();
            tab.delete_selection();
            true
        } else {
            false
        }
    }

    /// Clipboard Copy
    pub fn copy(&mut self) -> bool {
        let tab = &self.tabs[self.active_tab_index];
        if let Some(sel) = tab.selected_text() {
            self.clipboard = sel.to_string();
            true
        } else {
            false
        }
    }

    /// Clipboard Paste
    pub fn paste(&mut self) {
        if !self.clipboard.is_empty() {
            let clip = self.clipboard.clone();
            self.tabs[self.active_tab_index].insert_str(&clip);
        }
    }

    /// Insert current formatted Time and Date (F5)
    pub fn insert_time_date(&mut self, date_time_str: &str) {
        self.tabs[self.active_tab_index].insert_str(date_time_str);
    }

    /// Format status bar string: "Ln 12, Col 4 | 100% | Windows (CRLF) | UTF-8"
    pub fn status_bar_text(&self) -> String {
        let tab = self.active_tab();
        let (line, col) = tab.cursor_line_col();
        format!(
            "Ln {}, Col {} | {}% | {} | {}",
            line,
            col,
            tab.zoom_level,
            tab.line_ending.display_name(),
            tab.encoding.display_name()
        )
    }

    /// Save active document
    pub fn save_active_tab(&mut self) -> Result<String, &'static str> {
        let tab = &mut self.tabs[self.active_tab_index];
        if let Some(ref path) = tab.file_path {
            tab.is_dirty = false;
            Ok(path.clone())
        } else {
            Err("Document has no file path. Use Save As.")
        }
    }

    /// Save As with new file path
    pub fn save_as_active_tab(&mut self, new_path: &str) {
        let filename = std::path::Path::new(new_path)
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Document".to_string());
        let tab = &mut self.tabs[self.active_tab_index];
        tab.file_path = Some(new_path.to_string());
        tab.title = filename;
        tab.is_dirty = false;
    }
}

// ============================================================================
// Section 3: Windows 11 Modern Calculator
// ============================================================================

/// Calculator application modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculatorMode {
    Standard,
    Scientific,
    Programmer,
}

/// Binary arithmetic operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Modulo,
}

impl StandardOperator {
    pub fn symbol(&self) -> &'static str {
        match self {
            StandardOperator::Add => "+",
            StandardOperator::Subtract => "-",
            StandardOperator::Multiply => "*",
            StandardOperator::Divide => "/",
            StandardOperator::Power => "^",
            StandardOperator::Modulo => "Mod",
        }
    }

    pub fn apply(&self, left: f64, right: f64) -> Result<f64, &'static str> {
        match self {
            StandardOperator::Add => Ok(left + right),
            StandardOperator::Subtract => Ok(left - right),
            StandardOperator::Multiply => Ok(left * right),
            StandardOperator::Divide => {
                if right.abs() < 1e-15 {
                    Err("Cannot divide by zero")
                } else {
                    Ok(left / right)
                }
            }
            StandardOperator::Power => Ok(left.powf(right)),
            StandardOperator::Modulo => {
                if right.abs() < 1e-15 {
                    Err("Cannot divide by zero")
                } else {
                    Ok(left % right)
                }
            }
        }
    }
}

/// Completed calculation history item
#[derive(Debug, Clone, PartialEq)]
pub struct CalculatorHistoryEntry {
    pub equation: String,
    pub result: String,
}

/// Programmer mode bit word sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammerWordSize {
    Qword, // 64-bit
    Dword, // 32-bit
    Word,  // 16-bit
    Byte,  // 8-bit
}

impl ProgrammerWordSize {
    pub fn bit_count(&self) -> usize {
        match self {
            ProgrammerWordSize::Qword => 64,
            ProgrammerWordSize::Dword => 32,
            ProgrammerWordSize::Word => 16,
            ProgrammerWordSize::Byte => 8,
        }
    }

    pub fn mask(&self) -> u64 {
        match self {
            ProgrammerWordSize::Qword => !0u64,
            ProgrammerWordSize::Dword => 0x0000_0000_FFFF_FFFFu64,
            ProgrammerWordSize::Word => 0x0000_0000_0000_FFFFu64,
            ProgrammerWordSize::Byte => 0x0000_0000_0000_00FFu64,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProgrammerWordSize::Qword => "QWORD (64-bit)",
            ProgrammerWordSize::Dword => "DWORD (32-bit)",
            ProgrammerWordSize::Word => "WORD (16-bit)",
            ProgrammerWordSize::Byte => "BYTE (8-bit)",
        }
    }
}

/// Programmer mode numeric bases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammerBase {
    Hex,
    Dec,
    Oct,
    Bin,
}

/// Programmer bitwise operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitwiseOperator {
    And,
    Or,
    Xor,
    Lsh,
    Rsh,
    Ash,
    RoL,
    RoR,
}

impl BitwiseOperator {
    pub fn symbol(&self) -> &'static str {
        match self {
            BitwiseOperator::And => "AND",
            BitwiseOperator::Or => "OR",
            BitwiseOperator::Xor => "XOR",
            BitwiseOperator::Lsh => "Lsh",
            BitwiseOperator::Rsh => "Rsh",
            BitwiseOperator::Ash => "Ash",
            BitwiseOperator::RoL => "RoL",
            BitwiseOperator::RoR => "RoR",
        }
    }

    pub fn apply(&self, left: u64, right: u64, word_size: ProgrammerWordSize) -> u64 {
        let mask = word_size.mask();
        let bits = word_size.bit_count() as u32;
        let shift = (right as u32) % bits;

        let result = match self {
            BitwiseOperator::And => left & right,
            BitwiseOperator::Or => left | right,
            BitwiseOperator::Xor => left ^ right,
            BitwiseOperator::Lsh => left << shift,
            BitwiseOperator::Rsh => left >> shift,
            BitwiseOperator::Ash => {
                match word_size {
                    ProgrammerWordSize::Qword => ((left as i64) >> shift) as u64,
                    ProgrammerWordSize::Dword => ((left as i32) >> shift) as u32 as u64,
                    ProgrammerWordSize::Word => ((left as i16) >> shift) as u16 as u64,
                    ProgrammerWordSize::Byte => ((left as i8) >> shift) as u8 as u64,
                }
            }
            BitwiseOperator::RoL => {
                let l = left & mask;
                (l << shift) | (l >> (bits - shift))
            }
            BitwiseOperator::RoR => {
                let l = left & mask;
                (l >> shift) | (l << (bits - shift))
            }
        };

        result & mask
    }
}

/// Angle measurement unit for Scientific mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Degrees,
    Radians,
    Gradians,
}

impl AngleUnit {
    pub fn to_radians(&self, val: f64) -> f64 {
        match self {
            AngleUnit::Degrees => val * std::f64::consts::PI / 180.0,
            AngleUnit::Radians => val,
            AngleUnit::Gradians => val * std::f64::consts::PI / 200.0,
        }
    }

    pub fn from_radians(&self, rad: f64) -> f64 {
        match self {
            AngleUnit::Degrees => rad * 180.0 / std::f64::consts::PI,
            AngleUnit::Radians => rad,
            AngleUnit::Gradians => rad * 200.0 / std::f64::consts::PI,
        }
    }
}

/// Windows 11 Modern Calculator Implementation
#[derive(Debug, Clone)]
pub struct ModernCalculator {
    pub mode: CalculatorMode,
    pub display: String,
    pub active_equation: String,
    pub accumulator: Option<f64>,
    pub pending_operator: Option<StandardOperator>,
    pub is_new_entry: bool,
    pub history: Vec<CalculatorHistoryEntry>,
    pub memory_stack: Vec<f64>,

    // Programmer mode state
    pub programmer_value: u64,
    pub programmer_word_size: ProgrammerWordSize,
    pub programmer_base: ProgrammerBase,
    pub programmer_accumulator: Option<u64>,
    pub programmer_pending_op: Option<BitwiseOperator>,

    // Scientific mode state
    pub angle_unit: AngleUnit,
    pub expression_stack: Vec<(f64, StandardOperator)>,
}

impl ModernCalculator {
    pub fn new() -> Self {
        Self {
            mode: CalculatorMode::Standard,
            display: "0".to_string(),
            active_equation: String::new(),
            accumulator: None,
            pending_operator: None,
            is_new_entry: true,
            history: Vec::new(),
            memory_stack: Vec::new(),

            programmer_value: 0,
            programmer_word_size: ProgrammerWordSize::Qword,
            programmer_base: ProgrammerBase::Dec,
            programmer_accumulator: None,
            programmer_pending_op: None,

            angle_unit: AngleUnit::Degrees,
            expression_stack: Vec::new(),
        }
    }

    /// Switch active calculator mode
    pub fn set_mode(&mut self, mode: CalculatorMode) {
        self.mode = mode;
        self.clear();
    }

    // ------------------------------------------------------------------------
    // Standard Mode Operations
    // ------------------------------------------------------------------------

    /// Input single digit '0'..'9'
    pub fn input_digit(&mut self, digit: u8) {
        if self.mode == CalculatorMode::Programmer {
            self.input_programmer_digit(digit as u64);
            return;
        }

        if self.is_new_entry || self.display == "0" || self.display == "Error" {
            self.display = digit.to_string();
            self.is_new_entry = false;
        } else if self.display.len() < 16 {
            self.display.push((b'0' + digit) as char);
        }
    }

    /// Input decimal point '.'
    pub fn input_decimal(&mut self) {
        if self.mode == CalculatorMode::Programmer {
            return; // No decimals in programmer mode
        }

        if self.is_new_entry || self.display == "Error" {
            self.display = "0.".to_string();
            self.is_new_entry = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    /// Clear all (C button)
    pub fn clear(&mut self) {
        self.display = "0".to_string();
        self.active_equation.clear();
        self.accumulator = None;
        self.pending_operator = None;
        self.is_new_entry = true;

        self.programmer_value = 0;
        self.programmer_accumulator = None;
        self.programmer_pending_op = None;
        self.expression_stack.clear();
    }

    /// Clear Entry (CE button)
    pub fn clear_entry(&mut self) {
        self.display = "0".to_string();
        self.is_new_entry = true;
        if self.mode == CalculatorMode::Programmer {
            self.programmer_value = 0;
        }
    }

    /// Backspace (⌫ button)
    pub fn backspace(&mut self) {
        if self.mode == CalculatorMode::Programmer {
            self.programmer_value /= match self.programmer_base {
                ProgrammerBase::Hex => 16,
                ProgrammerBase::Dec => 10,
                ProgrammerBase::Oct => 8,
                ProgrammerBase::Bin => 2,
            };
            self.display = self.format_programmer_value();
            return;
        }

        if self.is_new_entry || self.display == "Error" {
            return;
        }

        self.display.pop();
        if self.display.is_empty() || self.display == "-" {
            self.display = "0".to_string();
            self.is_new_entry = true;
        }
    }

    /// Negate value (+/- button)
    pub fn negate(&mut self) {
        if self.mode == CalculatorMode::Programmer {
            let mask = self.programmer_word_size.mask();
            self.programmer_value = (!self.programmer_value + 1) & mask;
            self.display = self.format_programmer_value();
            return;
        }

        if self.display == "0" || self.display == "Error" {
            return;
        }

        if self.display.starts_with('-') {
            self.display.remove(0);
        } else {
            self.display.insert(0, '-');
        }
    }

    /// Set binary arithmetic operator (+, -, *, /, etc.)
    pub fn set_operator(&mut self, op: StandardOperator) {
        let current_val = self.parse_display();

        if let Some(accum) = self.accumulator {
            if let Some(pending) = self.pending_operator {
                if !self.is_new_entry {
                    match pending.apply(accum, current_val) {
                        Ok(res) => {
                            self.accumulator = Some(res);
                            self.display = Self::format_number(res);
                        }
                        Err(e) => {
                            self.display = e.to_string();
                            self.is_new_entry = true;
                            return;
                        }
                    }
                }
            }
        } else {
            self.accumulator = Some(current_val);
        }

        self.pending_operator = Some(op);
        let accum_str = Self::format_number(self.accumulator.unwrap());
        self.active_equation = format!("{} {}", accum_str, op.symbol());
        self.is_new_entry = true;
    }

    /// Equals (= button)
    pub fn equals(&mut self) {
        if self.mode == CalculatorMode::Programmer {
            self.programmer_equals();
            return;
        }

        if let (Some(accum), Some(op)) = (self.accumulator, self.pending_operator) {
            let current_val = self.parse_display();
            match op.apply(accum, current_val) {
                Ok(result) => {
                    let equation = format!("{} {} {} =", Self::format_number(accum), op.symbol(), Self::format_number(current_val));
                    let result_str = Self::format_number(result);

                    self.history.push(CalculatorHistoryEntry {
                        equation: equation.clone(),
                        result: result_str.clone(),
                    });

                    self.active_equation = equation;
                    self.display = result_str;
                    self.accumulator = None;
                    self.pending_operator = None;
                    self.is_new_entry = true;
                }
                Err(err) => {
                    self.display = err.to_string();
                    self.accumulator = None;
                    self.pending_operator = None;
                    self.is_new_entry = true;
                }
            }
        }
    }

    /// Percentage (% button)
    pub fn percentage(&mut self) {
        let val = self.parse_display();
        let result = if let Some(accum) = self.accumulator {
            accum * (val / 100.0)
        } else {
            val / 100.0
        };
        self.display = Self::format_number(result);
        self.is_new_entry = true;
    }

    /// Square root (√x)
    pub fn square_root(&mut self) {
        let val = self.parse_display();
        if val < 0.0 {
            self.display = "Invalid input".to_string();
        } else {
            let res = val.sqrt();
            self.active_equation = format!("sqrt({})", Self::format_number(val));
            self.display = Self::format_number(res);
        }
        self.is_new_entry = true;
    }

    /// Square (x²)
    pub fn square(&mut self) {
        let val = self.parse_display();
        let res = val * val;
        self.active_equation = format!("sqr({})", Self::format_number(val));
        self.display = Self::format_number(res);
        self.is_new_entry = true;
    }

    /// Reciprocal (1/x)
    pub fn reciprocal(&mut self) {
        let val = self.parse_display();
        if val.abs() < 1e-15 {
            self.display = "Cannot divide by zero".to_string();
        } else {
            let res = 1.0 / val;
            self.active_equation = format!("1/({})", Self::format_number(val));
            self.display = Self::format_number(res);
        }
        self.is_new_entry = true;
    }

    // ------------------------------------------------------------------------
    // Memory Management (MC, MR, M+, M-, MS, Mv)
    // ------------------------------------------------------------------------

    /// Memory Clear (MC)
    pub fn memory_clear(&mut self) {
        self.memory_stack.clear();
    }

    /// Memory Recall (MR)
    pub fn memory_recall(&mut self) {
        if let Some(&top) = self.memory_stack.last() {
            self.display = Self::format_number(top);
            self.is_new_entry = true;
        }
    }

    /// Memory Add (M+)
    pub fn memory_add(&mut self) {
        let val = self.parse_display();
        if self.memory_stack.is_empty() {
            self.memory_stack.push(val);
        } else {
            let last = self.memory_stack.last_mut().unwrap();
            *last += val;
        }
        self.is_new_entry = true;
    }

    /// Memory Subtract (M-)
    pub fn memory_subtract(&mut self) {
        let val = self.parse_display();
        if self.memory_stack.is_empty() {
            self.memory_stack.push(-val);
        } else {
            let last = self.memory_stack.last_mut().unwrap();
            *last -= val;
        }
        self.is_new_entry = true;
    }

    /// Memory Store (MS)
    pub fn memory_store(&mut self) {
        let val = self.parse_display();
        self.memory_stack.push(val);
        self.is_new_entry = true;
    }

    /// Has any memory stored (for button enable state)
    pub fn has_memory(&self) -> bool {
        !self.memory_stack.is_empty()
    }

    // ------------------------------------------------------------------------
    // Programmer Mode Operations
    // ------------------------------------------------------------------------

    /// Set word size (Qword, Dword, Word, Byte)
    pub fn set_programmer_word_size(&mut self, ws: ProgrammerWordSize) {
        self.programmer_word_size = ws;
        self.programmer_value &= ws.mask();
        self.display = self.format_programmer_value();
    }

    /// Set active base (Hex, Dec, Oct, Bin)
    pub fn set_programmer_base(&mut self, base: ProgrammerBase) {
        self.programmer_base = base;
        self.display = self.format_programmer_value();
    }

    /// Input digit for programmer base (0-15)
    pub fn input_programmer_digit(&mut self, digit: u64) {
        let max_digit = match self.programmer_base {
            ProgrammerBase::Hex => 15,
            ProgrammerBase::Dec => 9,
            ProgrammerBase::Oct => 7,
            ProgrammerBase::Bin => 1,
        };

        if digit > max_digit {
            return;
        }

        let base_mult = match self.programmer_base {
            ProgrammerBase::Hex => 16,
            ProgrammerBase::Dec => 10,
            ProgrammerBase::Oct => 8,
            ProgrammerBase::Bin => 2,
        };

        let mask = self.programmer_word_size.mask();
        let new_val = self.programmer_value.saturating_mul(base_mult).saturating_add(digit) & mask;
        self.programmer_value = new_val;
        self.display = self.format_programmer_value();
        self.is_new_entry = false;
    }

    /// Hex representation string
    pub fn hex_string(&self) -> String {
        let val = self.programmer_value & self.programmer_word_size.mask();
        format!("{:X}", val)
    }

    /// Dec representation string (signed/unsigned friendly)
    pub fn dec_string(&self) -> String {
        let val = self.programmer_value & self.programmer_word_size.mask();
        match self.programmer_word_size {
            ProgrammerWordSize::Qword => format!("{}", val as i64),
            ProgrammerWordSize::Dword => format!("{}", (val as u32) as i32),
            ProgrammerWordSize::Word => format!("{}", (val as u16) as i16),
            ProgrammerWordSize::Byte => format!("{}", (val as u8) as i8),
        }
    }

    /// Oct representation string
    pub fn oct_string(&self) -> String {
        let val = self.programmer_value & self.programmer_word_size.mask();
        format!("{:o}", val)
    }

    /// Bin representation string formatted in 4-bit nibbles
    pub fn bin_string(&self) -> String {
        let val = self.programmer_value & self.programmer_word_size.mask();
        let bits = self.programmer_word_size.bit_count();
        let mut out = String::with_capacity(bits + bits / 4);

        for i in (0..bits).rev() {
            let bit = (val >> i) & 1;
            out.push(if bit == 1 { '1' } else { '0' });
            if i > 0 && i % 4 == 0 {
                out.push(' ');
            }
        }
        out
    }

    /// Toggle a single bit in the 64-bit matrix (0 to 63)
    pub fn toggle_bit(&mut self, bit_index: usize) -> bool {
        if bit_index < self.programmer_word_size.bit_count() {
            let mask = 1u64 << bit_index;
            self.programmer_value ^= mask;
            self.programmer_value &= self.programmer_word_size.mask();
            self.display = self.format_programmer_value();
            true
        } else {
            false
        }
    }

    /// Get state of single bit (0 to 63)
    pub fn get_bit(&self, bit_index: usize) -> bool {
        if bit_index < 64 {
            ((self.programmer_value >> bit_index) & 1) == 1
        } else {
            false
        }
    }

    /// Set Programmer bitwise operator
    pub fn set_bitwise_operator(&mut self, op: BitwiseOperator) {
        if let Some(accum) = self.programmer_accumulator {
            if let Some(pending) = self.programmer_pending_op {
                let res = pending.apply(accum, self.programmer_value, self.programmer_word_size);
                self.programmer_accumulator = Some(res);
                self.programmer_value = res;
            }
        } else {
            self.programmer_accumulator = Some(self.programmer_value);
        }

        self.programmer_pending_op = Some(op);
        self.active_equation = format!("{} {}", self.format_programmer_value(), op.symbol());
        self.programmer_value = 0;
        self.display = self.format_programmer_value();
    }

    /// Bitwise NOT operator (immediate unary)
    pub fn bitwise_not(&mut self) {
        let mask = self.programmer_word_size.mask();
        self.programmer_value = (!self.programmer_value) & mask;
        self.active_equation = format!("NOT({})", self.format_programmer_value());
        self.display = self.format_programmer_value();
    }

    fn programmer_equals(&mut self) {
        if let (Some(accum), Some(op)) = (self.programmer_accumulator, self.programmer_pending_op) {
            let result = op.apply(accum, self.programmer_value, self.programmer_word_size);
            let equation = format!("{} {} {} =", accum, op.symbol(), self.programmer_value);

            self.history.push(CalculatorHistoryEntry {
                equation: equation.clone(),
                result: result.to_string(),
            });

            self.active_equation = equation;
            self.programmer_value = result;
            self.display = self.format_programmer_value();
            self.programmer_accumulator = None;
            self.programmer_pending_op = None;
        }
    }

    fn format_programmer_value(&self) -> String {
        match self.programmer_base {
            ProgrammerBase::Hex => self.hex_string(),
            ProgrammerBase::Dec => self.dec_string(),
            ProgrammerBase::Oct => self.oct_string(),
            ProgrammerBase::Bin => self.bin_string(),
        }
    }

    // ------------------------------------------------------------------------
    // Scientific Mode Operations
    // ------------------------------------------------------------------------

    pub fn sin(&mut self) {
        let val = self.parse_display();
        let rad = self.angle_unit.to_radians(val);
        let res = rad.sin();
        self.active_equation = format!("sin({})", val);
        self.display = Self::format_number(res);
        self.is_new_entry = true;
    }

    pub fn cos(&mut self) {
        let val = self.parse_display();
        let rad = self.angle_unit.to_radians(val);
        let res = rad.cos();
        self.active_equation = format!("cos({})", val);
        self.display = Self::format_number(res);
        self.is_new_entry = true;
    }

    pub fn tan(&mut self) {
        let val = self.parse_display();
        let rad = self.angle_unit.to_radians(val);
        let res = rad.tan();
        self.active_equation = format!("tan({})", val);
        self.display = Self::format_number(res);
        self.is_new_entry = true;
    }

    pub fn ln(&mut self) {
        let val = self.parse_display();
        if val <= 0.0 {
            self.display = "Invalid input".to_string();
        } else {
            let res = val.ln();
            self.active_equation = format!("ln({})", val);
            self.display = Self::format_number(res);
        }
        self.is_new_entry = true;
    }

    pub fn log10(&mut self) {
        let val = self.parse_display();
        if val <= 0.0 {
            self.display = "Invalid input".to_string();
        } else {
            let res = val.log10();
            self.active_equation = format!("log({})", val);
            self.display = Self::format_number(res);
        }
        self.is_new_entry = true;
    }

    pub fn factorial(&mut self) {
        let val = self.parse_display();
        if val < 0.0 || val.fract() != 0.0 || val > 170.0 {
            self.display = "Invalid input".to_string();
        } else {
            let n = val as u64;
            let mut fact = 1.0f64;
            for i in 2..=n {
                fact *= i as f64;
            }
            self.active_equation = format!("fact({})", n);
            self.display = Self::format_number(fact);
        }
        self.is_new_entry = true;
    }

    pub fn input_pi(&mut self) {
        self.display = std::f64::consts::PI.to_string();
        self.is_new_entry = true;
    }

    pub fn input_e(&mut self) {
        self.display = std::f64::consts::E.to_string();
        self.is_new_entry = true;
    }

    // ------------------------------------------------------------------------
    // Helper Formatters
    // ------------------------------------------------------------------------

    fn parse_display(&self) -> f64 {
        self.display.parse::<f64>().unwrap_or(0.0)
    }

    pub fn format_number(val: f64) -> String {
        if val.is_nan() {
            "NaN".to_string()
        } else if val.is_infinite() {
            if val > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
        } else if val.fract() == 0.0 && val.abs() < 1e14 {
            format!("{}", val as i64)
        } else {
            // Trim excessive trailing zeros
            let s = format!("{:.10}", val);
            let trimmed = s.trim_end_matches('0').trim_end_matches('.');
            trimmed.to_string()
        }
    }
}

// ============================================================================
// Section 4: Windows 11 Snipping Tool
// ============================================================================

/// Capture modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnipMode {
    Rectangle,
    Freeform,
    Window,
    Fullscreen,
}

impl SnipMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            SnipMode::Rectangle => "Rectangle Snip",
            SnipMode::Freeform => "Freeform Snip",
            SnipMode::Window => "Window Snip",
            SnipMode::Fullscreen => "Fullscreen Snip",
        }
    }
}

/// Delay timer selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnipDelay {
    NoDelay,
    ThreeSeconds,
    FiveSeconds,
    TenSeconds,
}

impl SnipDelay {
    pub fn seconds(&self) -> u32 {
        match self {
            SnipDelay::NoDelay => 0,
            SnipDelay::ThreeSeconds => 3,
            SnipDelay::FiveSeconds => 5,
            SnipDelay::TenSeconds => 10,
        }
    }
}

/// Snipping tool operational state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnipState {
    Idle,
    CountingDown { remaining_seconds: u32 },
    SelectingRegion,
    Annotating,
}

/// Annotation tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationToolType {
    Pen,
    Highlighter,
    Eraser,
    Ruler,
    Protractor,
}

/// Vector stroke representation on annotation canvas
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationStroke {
    pub tool: AnnotationToolType,
    pub color: Color,
    pub thickness: u32,
    pub points: Vec<Point>,
}

/// Straight-edge ruler guide state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerGuide {
    pub origin: Point,
    pub angle_radians: f64,
    pub length: u32,
    pub visible: bool,
}

/// 360-degree protractor guide state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProtractorGuide {
    pub center: Point,
    pub radius: u32,
    pub visible: bool,
}

/// Export image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Bmp,
    Jpg,
}

/// Share destination targets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareTarget {
    Clipboard,
    Email,
    Teams,
    NearbyShare,
}

/// Share payload package
#[derive(Debug, Clone)]
pub struct SharePayload {
    pub title: String,
    pub mime_type: &'static str,
    pub byte_payload: Vec<u8>,
}

/// Clipboard bitmap payload
#[derive(Debug, Clone)]
pub struct SnipClipboardData {
    pub width: u32,
    pub height: u32,
    pub rgba_bytes: Vec<u8>,
}

/// Raw RGBA bitmap buffer for capture and canvas rendering
#[derive(Debug, Clone, PartialEq)]
pub struct SnipBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA 4 bytes per pixel
}

impl SnipBitmap {
    pub fn new(width: u32, height: u32, initial_color: Color) -> Self {
        let total_bytes = (width as usize) * (height as usize) * 4;
        let mut pixels = Vec::with_capacity(total_bytes);
        for _ in 0..(width * height) {
            pixels.push(initial_color.r);
            pixels.push(initial_color.g);
            pixels.push(initial_color.b);
            pixels.push(initial_color.a);
        }
        Self { width, height, pixels }
    }

    pub fn from_rgba(width: u32, height: u32, bytes: Vec<u8>) -> Self {
        assert_eq!(bytes.len(), (width as usize) * (height as usize) * 4);
        Self { width, height, pixels: bytes }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x < self.width && y < self.height {
            let offset = ((y * self.width + x) * 4) as usize;
            Some(Color::rgba(
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
                self.pixels[offset + 3],
            ))
        } else {
            None
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            let offset = ((y * self.width + x) * 4) as usize;
            self.pixels[offset] = color.r;
            self.pixels[offset + 1] = color.g;
            self.pixels[offset + 2] = color.b;
            self.pixels[offset + 3] = color.a;
        }
    }

    /// Crop sub-rectangle from bitmap
    pub fn crop(&self, rect: Rect) -> SnipBitmap {
        let x1 = rect.left().max(0) as u32;
        let y1 = rect.top().max(0) as u32;
        let x2 = (rect.right().max(0) as u32).min(self.width);
        let y2 = (rect.bottom().max(0) as u32).min(self.height);

        if x2 <= x1 || y2 <= y1 {
            return SnipBitmap::new(1, 1, Color::TRANSPARENT);
        }

        let new_w = x2 - x1;
        let new_h = y2 - y1;
        let mut out = SnipBitmap::new(new_w, new_h, Color::TRANSPARENT);

        for y in 0..new_h {
            for x in 0..new_w {
                if let Some(c) = self.get_pixel(x1 + x, y1 + y) {
                    out.set_pixel(x, y, c);
                }
            }
        }

        out
    }

    /// Freeform snip mask: retains pixels inside polygon, makes outside transparent
    pub fn mask_polygon(&self, polygon: &[Point]) -> SnipBitmap {
        if polygon.len() < 3 {
            return self.clone();
        }

        let mut out = self.clone();

        // Ray casting algorithm for point in polygon
        let point_in_polygon = |px: i32, py: i32| -> bool {
            let mut inside = false;
            let mut j = polygon.len() - 1;
            for i in 0..polygon.len() {
                let pi = polygon[i];
                let pj = polygon[j];

                if ((pi.y > py) != (pj.y > py)) &&
                    (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x) {
                    inside = !inside;
                }
                j = i;
            }
            inside
        };

        for y in 0..self.height {
            for x in 0..self.width {
                if !point_in_polygon(x as i32, y as i32) {
                    out.set_pixel(x, y, Color::TRANSPARENT);
                }
            }
        }

        out
    }

    /// Draw straight or brushed line with thickness and anti-aliasing
    pub fn draw_line(&mut self, p1: Point, p2: Point, color: Color, thickness: u32) {
        let radius = (thickness as f32 / 2.0).max(0.5);
        let dist = p1.distance(p2);
        let steps = (dist * 2.0).ceil() as usize;

        for step in 0..=steps {
            let t = if steps == 0 { 0.0 } else { step as f64 / steps as f64 };
            let cx = (p1.x as f64 + (p2.x - p1.x) as f64 * t).round() as i32;
            let cy = (p1.y as f64 + (p2.y - p1.y) as f64 * t).round() as i32;
            self.draw_brush_circle(Point::new(cx, cy), radius, color);
        }
    }

    /// Draw solid brush disk
    pub fn draw_brush_circle(&mut self, center: Point, radius: f32, color: Color) {
        let r_int = radius.ceil() as i32;
        let r_sq = radius * radius;

        let min_x = (center.x - r_int).max(0) as u32;
        let max_x = ((center.x + r_int).max(0) as u32).min(self.width.saturating_sub(1));
        let min_y = (center.y - r_int).max(0) as u32;
        let max_y = ((center.y + r_int).max(0) as u32).min(self.height.saturating_sub(1));

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - center.x as f32;
                let dy = y as f32 - center.y as f32;
                let d_sq = dx * dx + dy * dy;

                if d_sq <= r_sq {
                    let cur = self.get_pixel(x, y).unwrap_or(Color::TRANSPARENT);
                    let blended = color.blend_over(cur);
                    self.set_pixel(x, y, blended);
                }
            }
        }
    }
}

/// Windows 11 Modern Snipping Tool Application
#[derive(Debug, Clone)]
pub struct ModernSnippingTool {
    pub mode: SnipMode,
    pub delay: SnipDelay,
    pub state: SnipState,
    pub active_tool: AnnotationToolType,
    pub pen_color: Color,
    pub pen_thickness: u32,
    pub highlighter_color: Color,
    pub highlighter_thickness: u32,
    pub ruler: RulerGuide,
    pub protractor: ProtractorGuide,

    // Captured canvas
    pub base_capture: Option<SnipBitmap>,
    pub annotations: Vec<AnnotationStroke>,
    pub undo_annotations: Vec<AnnotationStroke>,
}

impl ModernSnippingTool {
    pub fn new() -> Self {
        Self {
            mode: SnipMode::Rectangle,
            delay: SnipDelay::NoDelay,
            state: SnipState::Idle,
            active_tool: AnnotationToolType::Pen,
            pen_color: Color::RED,
            pen_thickness: 3,
            highlighter_color: Color::HIGHLIGHT_YELLOW,
            highlighter_thickness: 18,
            ruler: RulerGuide {
                origin: Point::new(100, 100),
                angle_radians: 0.0,
                length: 400,
                visible: false,
            },
            protractor: ProtractorGuide {
                center: Point::new(200, 200),
                radius: 120,
                visible: false,
            },
            base_capture: None,
            annotations: Vec::new(),
            undo_annotations: Vec::new(),
        }
    }

    /// Trigger "New" snip with active delay
    pub fn start_new_snip(&mut self) {
        let delay_sec = self.delay.seconds();
        if delay_sec == 0 {
            self.state = SnipState::SelectingRegion;
        } else {
            self.state = SnipState::CountingDown { remaining_seconds: delay_sec };
        }
    }

    /// Countdown timer tick (called every 1 second)
    pub fn tick_timer(&mut self) -> bool {
        if let SnipState::CountingDown { ref mut remaining_seconds } = self.state {
            if *remaining_seconds > 1 {
                *remaining_seconds -= 1;
                false
            } else {
                self.state = SnipState::SelectingRegion;
                true // Transitioned to active snip
            }
        } else {
            false
        }
    }

    /// Complete snip region selection from full desktop buffer
    pub fn complete_snip(&mut self, desktop_screen: &SnipBitmap, region: Rect) {
        let cropped = match self.mode {
            SnipMode::Rectangle | SnipMode::Window => desktop_screen.crop(region),
            SnipMode::Fullscreen => desktop_screen.clone(),
            SnipMode::Freeform => desktop_screen.crop(region),
        };

        self.base_capture = Some(cropped);
        self.annotations.clear();
        self.undo_annotations.clear();
        self.state = SnipState::Annotating;
    }

    /// Add a freehand vector stroke
    pub fn add_stroke(&mut self, points: Vec<Point>) {
        if points.is_empty() {
            return;
        }

        let stroke = match self.active_tool {
            AnnotationToolType::Pen => AnnotationStroke {
                tool: AnnotationToolType::Pen,
                color: self.pen_color,
                thickness: self.pen_thickness,
                points,
            },
            AnnotationToolType::Highlighter => AnnotationStroke {
                tool: AnnotationToolType::Highlighter,
                color: self.highlighter_color,
                thickness: self.highlighter_thickness,
                points,
            },
            AnnotationToolType::Eraser => {
                self.erase_near_points(&points, 16);
                return;
            }
            _ => return,
        };

        self.annotations.push(stroke);
        self.undo_annotations.clear();
    }

    /// Erase annotations that come within radius of eraser stroke points
    pub fn erase_near_points(&mut self, eraser_points: &[Point], radius: u32) {
        let r_sq = (radius * radius) as i64;
        self.annotations.retain(|stroke| {
            !stroke.points.iter().any(|p1| {
                eraser_points.iter().any(|p2| p1.distance_squared(*p2) <= r_sq)
            })
        });
    }

    /// Undo last annotation stroke
    pub fn undo_annotation(&mut self) -> bool {
        if let Some(stroke) = self.annotations.pop() {
            self.undo_annotations.push(stroke);
            true
        } else {
            false
        }
    }

    /// Redo previously undone annotation stroke
    pub fn redo_annotation(&mut self) -> bool {
        if let Some(stroke) = self.undo_annotations.pop() {
            self.annotations.push(stroke);
            true
        } else {
            false
        }
    }

    /// Composite base image + annotations into final flat bitmap
    pub fn render_composite(&self) -> Option<SnipBitmap> {
        let base = self.base_capture.as_ref()?;
        let mut composite = base.clone();

        for stroke in &self.annotations {
            if stroke.points.len() == 1 {
                composite.draw_brush_circle(stroke.points[0], stroke.thickness as f32 / 2.0, stroke.color);
            } else {
                for i in 0..stroke.points.len().saturating_sub(1) {
                    composite.draw_line(stroke.points[i], stroke.points[i + 1], stroke.color, stroke.thickness);
                }
            }
        }

        Some(composite)
    }

    /// Copy to clipboard
    pub fn copy_to_clipboard(&self) -> Option<SnipClipboardData> {
        let comp = self.render_composite()?;
        Some(SnipClipboardData {
            width: comp.width,
            height: comp.height,
            rgba_bytes: comp.pixels,
        })
    }

    /// Export to pure zero-dependency PNG bytes (standard valid PNG chunks: IHDR, IDAT, IEND)
    pub fn export_png(&self) -> Result<Vec<u8>, &'static str> {
        let comp = self.render_composite().ok_or("No active capture to export")?;
        encode_png_rgba(comp.width, comp.height, &comp.pixels)
    }

    /// Export to 32-bit BMP bytes
    pub fn export_bmp(&self) -> Result<Vec<u8>, &'static str> {
        let comp = self.render_composite().ok_or("No active capture to export")?;
        encode_bmp_rgba(comp.width, comp.height, &comp.pixels)
    }

    /// Share payload dispatcher
    pub fn share(&self, target: ShareTarget) -> Result<SharePayload, &'static str> {
        let png_bytes = self.export_png()?;
        let title = match target {
            ShareTarget::Clipboard => "Screenshot Copied to Clipboard".to_string(),
            ShareTarget::Email => "Snip - Email Attachment".to_string(),
            ShareTarget::Teams => "Snip - Microsoft Teams".to_string(),
            ShareTarget::NearbyShare => "Snip - Windows Nearby Sharing".to_string(),
        };

        Ok(SharePayload {
            title,
            mime_type: "image/png",
            byte_payload: png_bytes,
        })
    }
}

// ============================================================================
// Section 5: Zero-Dependency Pure Rust PNG & BMP Encoders
// ============================================================================

/// CRC32 IEEE 802.3 generator
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Adler-32 checksum for Zlib stream
fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &byte in data {
        s1 = (s1 + byte as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

/// Encode RGBA bytes into standard compliant PNG file format
pub fn encode_png_rgba(width: u32, height: u32, rgba_pixels: &[u8]) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 {
        return Err("Invalid image dimensions");
    }

    let mut out = Vec::new();
    // 1. PNG Signature
    out.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // 2. IHDR Chunk (13 bytes)
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // Bit depth
    ihdr_data.push(6); // Color type: RGBA
    ihdr_data.push(0); // Compression method: deflate
    ihdr_data.push(0); // Filter method: standard
    ihdr_data.push(0); // Interlace method: none

    write_png_chunk(&mut out, b"IHDR", &ihdr_data);

    // 3. IDAT Chunk: Raw scanlines with Filter 0 (None) + uncompressed Deflate zlib stream
    let scanline_len = (width as usize) * 4;
    let mut raw_scanlines = Vec::with_capacity((scanline_len + 1) * (height as usize));

    for y in 0..(height as usize) {
        raw_scanlines.push(0u8); // Filter: None
        let row_start = y * scanline_len;
        raw_scanlines.extend_from_slice(&rgba_pixels[row_start..row_start + scanline_len]);
    }

    // Wrap raw scanlines in Zlib deflate envelope (uncompressed blocks, max 65535 bytes per block)
    let mut zlib_stream = Vec::new();
    zlib_stream.push(0x78); // CMF: Deflate, 32K window
    zlib_stream.push(0x01); // FLG: Check bits

    let chunks = raw_scanlines.chunks(65535);
    let total_chunks = chunks.len();

    for (idx, chunk) in chunks.enumerate() {
        let is_last = idx + 1 == total_chunks;
        zlib_stream.push(if is_last { 0x01 } else { 0x00 }); // BFINAL and BTYPE=00 (uncompressed)
        let len = chunk.len() as u16;
        let nlen = !len;
        zlib_stream.extend_from_slice(&len.to_le_bytes());
        zlib_stream.extend_from_slice(&nlen.to_le_bytes());
        zlib_stream.extend_from_slice(chunk);
    }

    // Adler32 of uncompressed scanlines
    let adler = adler32(&raw_scanlines);
    zlib_stream.extend_from_slice(&adler.to_be_bytes());

    write_png_chunk(&mut out, b"IDAT", &zlib_stream);

    // 4. IEND Chunk
    write_png_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);

    let mut crc_buf = Vec::with_capacity(4 + data.len());
    crc_buf.extend_from_slice(chunk_type);
    crc_buf.extend_from_slice(data);
    let crc = crc32(&crc_buf);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode RGBA bytes into standard 32-bit BMP file format
pub fn encode_bmp_rgba(width: u32, height: u32, rgba_pixels: &[u8]) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 {
        return Err("Invalid image dimensions");
    }

    let file_header_size = 14u32;
    let dib_header_size = 40u32; // BITMAPINFOHEADER
    let image_size = width * height * 4;
    let file_size = file_header_size + dib_header_size + image_size;

    let mut out = Vec::with_capacity(file_size as usize);

    // File header
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    out.extend_from_slice(&(file_header_size + dib_header_size).to_le_bytes()); // Offset to pixel data

    // DIB header (BITMAPINFOHEADER)
    out.extend_from_slice(&dib_header_size.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(-(height as i32)).to_le_bytes()); // Negative for top-down bitmap
    out.extend_from_slice(&1u16.to_le_bytes()); // Color planes
    out.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (uncompressed)
    out.extend_from_slice(&image_size.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // Horizontal resolution (72 DPI)
    out.extend_from_slice(&2835u32.to_le_bytes()); // Vertical resolution (72 DPI)
    out.extend_from_slice(&0u32.to_le_bytes()); // Colors in palette
    out.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // Pixel data in BGRA format
    for chunk in rgba_pixels.chunks_exact(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let a = chunk[3];
        out.push(b);
        out.push(g);
        out.push(r);
        out.push(a);
    }

    Ok(out)
}

// ============================================================================
// Section 6: Unified Commercial Productivity Apps Manager
// ============================================================================

/// Master manager orchestrating Windows 11 built-in productivity tools
#[derive(Debug, Clone)]
pub struct ProductivityAppsManager {
    pub notepad: ModernNotepad,
    pub calculator: ModernCalculator,
    pub snipping_tool: ModernSnippingTool,
}

impl ProductivityAppsManager {
    pub fn new() -> Self {
        Self {
            notepad: ModernNotepad::new(),
            calculator: ModernCalculator::new(),
            snipping_tool: ModernSnippingTool::new(),
        }
    }
}

// ============================================================================
// Section 7: Comprehensive Unit Tests (8 Production Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notepad_text_editing_and_cursor() {
        let mut notepad = ModernNotepad::new();
        {
            let tab = notepad.active_tab_mut();
            tab.insert_str("Hello World\nLine 2 text");
            assert_eq!(tab.text, "Hello World\nLine 2 text");
            assert!(tab.is_dirty);

            let (line, col) = tab.cursor_line_col();
            assert_eq!(line, 2);
            assert_eq!(col, 12);

            let (chars, words, lines) = tab.statistics();
            assert_eq!(chars, 23);
            assert_eq!(words, 5);
            assert_eq!(lines, 2);

            // Test backspace
            assert!(tab.delete_backward());
            assert_eq!(tab.text, "Hello World\nLine 2 tex");

            // Test delete forward
            tab.cursor_byte_offset = 5;
            assert!(tab.delete_forward()); // Deletes space between Hello and World
            assert_eq!(tab.text, "HelloWorld\nLine 2 tex");
        }
    }

    #[test]
    fn test_notepad_undo_redo_and_dirty_state() {
        let mut tab = NotepadTab::new(1, "Test.txt".to_string());
        assert!(!tab.is_dirty);

        tab.insert_str("First edit. ");
        assert!(tab.is_dirty);
        assert_eq!(tab.text, "First edit. ");

        tab.insert_str("Second edit.");
        assert_eq!(tab.text, "First edit. Second edit.");

        // Undo second edit
        assert!(tab.undo());
        assert_eq!(tab.text, "First edit. ");

        // Undo first edit
        assert!(tab.undo());
        assert_eq!(tab.text, "");

        // Redo first edit
        assert!(tab.redo());
        assert_eq!(tab.text, "First edit. ");

        // Redo second edit
        assert!(tab.redo());
        assert_eq!(tab.text, "First edit. Second edit.");
    }

    #[test]
    fn test_notepad_find_replace_and_tabs() {
        let mut notepad = ModernNotepad::new();
        notepad.active_tab_mut().insert_str("Tab 1 initial notes.");
        let tab1_id = notepad.active_tab().id;

        // Add a second tab
        let tab2_id = notepad.open_file("C:\\Docs\\notes.txt", "Rust is safe.\nRust is fast.\nRust is modern.");
        assert_eq!(notepad.tabs.len(), 2);
        assert_eq!(notepad.active_tab().id, tab2_id);

        let opts = FindReplaceOptions {
            query: "Rust".to_string(),
            replacement: "HybridOS".to_string(),
            match_case: true,
            match_whole_word: true,
            wrap_around: true,
            direction: SearchDirection::Forward,
        };

        let replaced_count = notepad.active_tab_mut().replace_all(&opts);
        assert_eq!(replaced_count, 3);
        assert_eq!(
            notepad.active_tab().text,
            "HybridOS is safe.\nHybridOS is fast.\nHybridOS is modern."
        );

        // Switch back to tab 1
        assert!(notepad.switch_tab(tab1_id));
        assert_eq!(notepad.active_tab().id, tab1_id);
    }

    #[test]
    fn test_calculator_standard_operations_and_memory() {
        let mut calc = ModernCalculator::new();

        // 12 + 8 = 20
        calc.input_digit(1);
        calc.input_digit(2);
        calc.set_operator(StandardOperator::Add);
        calc.input_digit(8);
        calc.equals();
        assert_eq!(calc.display, "20");

        // Unary operations: square (20² = 400), square root (√400 = 20), reciprocal (1/20 = 0.05)
        calc.square();
        assert_eq!(calc.display, "400");
        calc.square_root();
        assert_eq!(calc.display, "20");
        calc.reciprocal();
        assert_eq!(calc.display, "0.05");

        // Memory Operations: MS(0.05), clear, recall
        calc.memory_store();
        assert!(calc.has_memory());
        calc.clear();
        assert_eq!(calc.display, "0");
        calc.memory_recall();
        assert_eq!(calc.display, "0.05");

        // Memory Add M+(10) -> 10.05
        calc.clear();
        calc.input_digit(1);
        calc.input_digit(0);
        calc.memory_add();
        calc.memory_recall();
        assert_eq!(calc.display, "10.05");

        calc.memory_clear();
        assert!(!calc.has_memory());
    }

    #[test]
    fn test_calculator_active_equation_and_history() {
        let mut calc = ModernCalculator::new();

        // Compute 125 + 45 = 170
        calc.input_digit(1);
        calc.input_digit(2);
        calc.input_digit(5);
        calc.set_operator(StandardOperator::Add);
        assert_eq!(calc.active_equation, "125 +");

        calc.input_digit(4);
        calc.input_digit(5);
        calc.equals();

        assert_eq!(calc.display, "170");
        assert_eq!(calc.active_equation, "125 + 45 =");
        assert_eq!(calc.history.len(), 1);
        assert_eq!(calc.history[0].equation, "125 + 45 =");
        assert_eq!(calc.history[0].result, "170");
    }

    #[test]
    fn test_calculator_programmer_bitwise_and_conversions() {
        let mut calc = ModernCalculator::new();
        calc.set_mode(CalculatorMode::Programmer);

        // Input decimal 254 (0xFE)
        calc.set_programmer_base(ProgrammerBase::Dec);
        calc.input_digit(2);
        calc.input_digit(5);
        calc.input_digit(4);

        assert_eq!(calc.hex_string(), "FE");
        assert_eq!(calc.dec_string(), "254");
        assert_eq!(calc.oct_string(), "376");

        // Bitwise AND with 0x0F (15) -> 0x0E (14)
        calc.set_bitwise_operator(BitwiseOperator::And);
        calc.input_digit(1);
        calc.input_digit(5);
        calc.equals();

        assert_eq!(calc.hex_string(), "E");
        assert_eq!(calc.dec_string(), "14");

        // Toggle bit 0 (14 ^ 1 = 15 = 0xF)
        assert!(calc.toggle_bit(0));
        assert_eq!(calc.hex_string(), "F");
        assert_eq!(calc.dec_string(), "15");

        // Test BYTE word size masking
        calc.set_programmer_word_size(ProgrammerWordSize::Byte);
        calc.bitwise_not(); // ~15 in 8-bit = 240 (0xF0)
        assert_eq!(calc.hex_string(), "F0");
    }

    #[test]
    fn test_calculator_scientific_mode() {
        let mut calc = ModernCalculator::new();
        calc.set_mode(CalculatorMode::Scientific);

        // sin(90°) in degrees = 1
        calc.input_digit(9);
        calc.input_digit(0);
        calc.sin();
        assert_eq!(calc.display, "1");

        // Factorial of 5 = 120
        calc.clear();
        calc.input_digit(5);
        calc.factorial();
        assert_eq!(calc.display, "120");

        // Constants Pi and E
        calc.input_pi();
        assert!(calc.display.starts_with("3.14159"));
    }

    #[test]
    fn test_snipping_tool_modes_and_canvas() {
        let mut tool = ModernSnippingTool::new();

        // Create a 100x100 mock desktop screen
        let desktop = SnipBitmap::new(100, 100, Color::WHITE);

        // Rectangle capture at (10, 10, 40, 40)
        tool.mode = SnipMode::Rectangle;
        tool.complete_snip(&desktop, Rect::new(10, 10, 40, 40));

        assert_eq!(tool.state, SnipState::Annotating);
        let capture = tool.base_capture.as_ref().unwrap();
        assert_eq!(capture.width, 40);
        assert_eq!(capture.height, 40);

        // Add a red pen stroke
        tool.active_tool = AnnotationToolType::Pen;
        tool.pen_color = Color::RED;
        tool.pen_thickness = 2;
        tool.add_stroke(vec![Point::new(5, 5), Point::new(35, 35)]);
        assert_eq!(tool.annotations.len(), 1);

        // Render composite bitmap
        let composite = tool.render_composite().expect("Render composite failed");
        assert_eq!(composite.width, 40);
        assert_eq!(composite.height, 40);

        // Verify valid PNG generation
        let png_bytes = tool.export_png().expect("PNG export failed");
        assert!(png_bytes.len() > 8);
        assert_eq!(&png_bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

        // Test delay timer
        tool.delay = SnipDelay::ThreeSeconds;
        tool.start_new_snip();
        assert_eq!(tool.state, SnipState::CountingDown { remaining_seconds: 3 });
        assert!(!tool.tick_timer()); // 2s remaining
        assert!(!tool.tick_timer()); // 1s remaining
        assert!(tool.tick_timer());  // Transition to SelectingRegion
        assert_eq!(tool.state, SnipState::SelectingRegion);
    }

    #[test]
    fn test_snipping_tool_freeform_polygon_masking() {
        let bitmap = SnipBitmap::new(20, 20, Color::RED);
        // Triangle polygon covering the top-left area
        let polygon = [Point::new(0, 0), Point::new(10, 0), Point::new(0, 10)];
        let masked = bitmap.mask_polygon(&polygon);

        // Point inside the triangle should remain red
        assert_eq!(masked.get_pixel(1, 1).unwrap(), Color::RED);

        // Point outside the triangle should become transparent
        assert_eq!(masked.get_pixel(15, 15).unwrap(), Color::TRANSPARENT);
    }

    #[test]
    fn test_calculator_programmer_shifts_and_rotates() {
        // Shift left
        let lsh = BitwiseOperator::Lsh.apply(0x01, 4, ProgrammerWordSize::Byte);
        assert_eq!(lsh, 0x10);

        // Logical shift right
        let rsh = BitwiseOperator::Rsh.apply(0x80, 2, ProgrammerWordSize::Byte);
        assert_eq!(rsh, 0x20);

        // Arithmetic shift right (sign extension)
        let ash = BitwiseOperator::Ash.apply(0x80, 1, ProgrammerWordSize::Byte);
        assert_eq!(ash, 0xC0); // 0b1100_0000

        // Rotate left
        let rol = BitwiseOperator::RoL.apply(0x81, 1, ProgrammerWordSize::Byte);
        assert_eq!(rol, 0x03); // 0b0000_0011

        // Rotate right
        let ror = BitwiseOperator::RoR.apply(0x01, 1, ProgrammerWordSize::Byte);
        assert_eq!(ror, 0x80); // 0b1000_0000
    }
}
