//! HybridOS Native Applications Suite — Category 5: Office & Productivity (Apps 34 to 41).
//!
//! Delivers high-fidelity Crystal Glass native productivity applications (#![no_std]):
//! 34. Bloco de Notas (Notepad): Multi-line text editor, blinking cursor, line/col counter, VFS read/write.
//! 35. WordPad: Centralized simulated A4 sheet, rich formatting (Bold, Italic, Underline, Colors, Alignments, Ruler).
//! 36. Calculadora Multi-Modo: Tabs [Padrão | Científica | Programador], history display, hex/dec/oct/bin, Word/Dword/Qword.
//! 37. Notas Autoadesivas (Sticky Notes): Floating colorful paper cards, color themes, auto-save to `/etc/stickies.txt`.
//! 38. Calendário, Relógio Mundial & Cronômetro: Interactive monthly grid, analog clock with hardware CMOS RTC, 100Hz centesimal stopwatch.
//! 39. Mapa de Caracteres & Font Viewer: Interactive 32..255 ASCII grid, 4x magnified preview, Dec/Hex info, clipboard copy.
//! 40. Gerenciador do Clipboard: Top 10 clipboard history slots with real preview, clear, select & paste.
//! 41. Catálogo de Contatos (Address Book): Contact manager with instant search, full form fields, export to `C:\contatos.csv`.
//!
//! Zero mocks, full interactive hitboxes, real VFS persistence, hardware CMOS & PIT 100Hz integration.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use crate::arch::idt::SYSTEM_TICKS;
use crate::arch::msr::{inb, outb};
use crate::compat::FloatExt;
use crate::drivers::desktop::{draw_text_clipped, draw_text_px, DesktopState};
use crate::drivers::gfx2d::*;
use crate::drivers::gop::FONT_8X16;
use crate::sync::SpinLock;
use crate::vfs;
use crate::vfs::inode::Permissions;

// ============================================================================
// Global Shared Clipboard System
// ============================================================================

pub static GLOBAL_CLIPBOARD: SpinLock<ClipboardHistory> = SpinLock::new(ClipboardHistory::new());

pub struct ClipboardHistory {
    pub items: Vec<String>,
    pub active_index: usize,
}

impl ClipboardHistory {
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
        }
    }

    pub fn push(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        // Remove duplicate if already present
        if let Some(pos) = self.items.iter().position(|it| it == &text) {
            self.items.remove(pos);
        }
        if self.items.len() >= 10 {
            self.items.pop();
        }
        self.items.insert(0, text);
        self.active_index = 0;
    }

    pub fn get_active(&self) -> Option<&String> {
        self.items.get(self.active_index)
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.active_index = 0;
    }
}

// ============================================================================
// State Structures for Category 5 Applications
// ============================================================================

pub struct NotepadState {
    pub text: String,
    pub cursor_pos: usize,
    pub search_query: String,
    pub search_open: bool,
    pub word_wrap: bool,
    pub status_msg: String,
    pub last_canvas: (usize, usize, usize, usize),
}

impl NotepadState {
    pub fn new() -> Self {
        let mut s = Self {
            text: String::from(
                "Bem-vindo ao Bloco de Notas do HybridOS!\n\
                 Este editor possui persistencia real no VFS bilingue.\n\
                 Voce pode digitar livremente, localizar termos e salvar em C:\\notepad.txt.\n",
            ),
            cursor_pos: 0,
            search_query: String::new(),
            search_open: false,
            word_wrap: false,
            status_msg: String::from("Pronto"),
            last_canvas: (0, 0, 0, 0),
        };
        s.cursor_pos = s.text.len();
        s
    }

    pub fn line_col(&self) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (idx, ch) in self.text.char_indices() {
            if idx >= self.cursor_pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    pub fn insert_char(&mut self, c: char) {
        if self.cursor_pos > self.text.len() {
            self.cursor_pos = self.text.len();
        }
        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.status_msg = String::from("Modificado");
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 && !self.text.is_empty() {
            let prev_pos = self.text[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.text.remove(prev_pos);
            self.cursor_pos = prev_pos;
            self.status_msg = String::from("Modificado");
        }
    }

    pub fn save_to_vfs(&mut self) {
        let _ = vfs::vfs_unlink("/notepad.txt");
        let res = vfs::vfs_create_file("/notepad.txt", self.text.as_bytes(), Permissions::default_file());
        match res {
            Ok(_) => {
                self.status_msg = String::from("Salvo com sucesso em C:\\notepad.txt");
            }
            Err(_) => {
                self.status_msg = String::from("Erro ao salvar no VFS");
            }
        }
    }

    pub fn load_from_vfs(&mut self) {
        if let Ok(bytes) = vfs::vfs_read_all("/notepad.txt") {
            if let Ok(s) = core::str::from_utf8(&bytes) {
                self.text = String::from(s);
                self.cursor_pos = self.text.len();
                self.status_msg = String::from("Carregado de C:\\notepad.txt");
            }
        } else {
            self.status_msg = String::from("Arquivo C:\\notepad.txt nao encontrado");
        }
    }
}

pub struct WordPadState {
    pub text: String,
    pub cursor_pos: usize,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color_idx: usize, // 0: Preto, 1: Azul, 2: Vermelho, 3: Verde
    pub align_mode: usize, // 0: Esquerda, 1: Centro, 2: Direita
    pub status_msg: String,
    pub last_canvas: (usize, usize, usize, usize),
}

impl WordPadState {
    pub fn new() -> Self {
        let mut s = Self {
            text: String::from(
                "DOCUMENTO FORMATADO HYBRIDOS\n\n\
                 Este processador de texto simula uma folha de papel A4 centralizada.\n\
                 Suporta estilizacao em tempo de execucao: Negrito, Italico, Sublinhado,\n\
                 paleta cromatica e alinhamento justificavel.\n\n\
                 Tudo integrado a camada de persistencia VFS.\n",
            ),
            cursor_pos: 0,
            bold: false,
            italic: false,
            underline: false,
            color_idx: 0,
            align_mode: 0,
            status_msg: String::from("Papel A4 Pronto"),
            last_canvas: (0, 0, 0, 0),
        };
        s.cursor_pos = s.text.len();
        s
    }

    pub fn current_color(&self) -> u32 {
        match self.color_idx {
            0 => 0x000F172A, // Preto profundo
            1 => 0x001D4ED8, // Azul real
            2 => 0x00DC2626, // Vermelho carmesim
            3 => 0x0016A34A, // Verde esmeralda
            _ => 0x000F172A,
        }
    }

    pub fn save_to_vfs(&mut self) {
        let _ = vfs::vfs_unlink("/documento.rtf");
        match vfs::vfs_create_file("/documento.rtf", self.text.as_bytes(), Permissions::default_file()) {
            Ok(_) => self.status_msg = String::from("Salvo em C:\\documento.rtf"),
            Err(_) => self.status_msg = String::from("Erro ao salvar C:\\documento.rtf"),
        }
    }

    pub fn load_from_vfs(&mut self) {
        if let Ok(bytes) = vfs::vfs_read_all("/documento.rtf") {
            if let Ok(s) = core::str::from_utf8(&bytes) {
                self.text = String::from(s);
                self.cursor_pos = self.text.len();
                self.status_msg = String::from("Carregado de C:\\documento.rtf");
            }
        }
    }
}

pub struct CalculatorState {
    pub mode: usize, // 0: Padrão, 1: Científica, 2: Programador
    pub display: String,
    pub history: String,
    pub acc: f64,
    pub pending_op: Option<char>,
    pub clear_on_next: bool,
    // Programmer mode
    pub prog_val: u64,
    pub prog_acc: u64,
    pub prog_op: Option<char>,
    pub prog_clear_on_next: bool,
    pub prog_word_size: usize, // 0: Qword (64), 1: Dword (32), 2: Word (16), 3: Byte (8)
    pub last_canvas: (usize, usize, usize, usize),
}

impl CalculatorState {
    pub fn new() -> Self {
        Self {
            mode: 0,
            display: String::from("0"),
            history: String::new(),
            acc: 0.0,
            pending_op: None,
            clear_on_next: false,
            prog_val: 0,
            prog_acc: 0,
            prog_op: None,
            prog_clear_on_next: false,
            prog_word_size: 0,
            last_canvas: (0, 0, 0, 0),
        }
    }

    pub fn input_digit(&mut self, d: char) {
        if self.clear_on_next || self.display == "0" {
            self.display.clear();
            self.clear_on_next = false;
        }
        if self.display.len() < 18 {
            self.display.push(d);
        }
        if let Ok(val) = self.display.parse::<f64>() {
            self.prog_val = (val as i64) as u64;
        }
    }

    pub fn input_dot(&mut self) {
        if self.clear_on_next {
            self.display = String::from("0");
            self.clear_on_next = false;
        }
        if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    pub fn set_op(&mut self, op: char) {
        if let Ok(val) = self.display.parse::<f64>() {
            if let Some(prev_op) = self.pending_op {
                self.acc = match prev_op {
                    '+' => self.acc + val,
                    '-' => self.acc - val,
                    '*' => self.acc * val,
                    '/' => if val != 0.0 { self.acc / val } else { 0.0 },
                    '^' => self.acc.powf(val),
                    _ => val,
                };
            } else {
                self.acc = val;
            }
            let acc_str = if self.acc.fract() == 0.0 && self.acc.abs() < 1e15 {
                format!("{}", self.acc as i64)
            } else {
                format!("{:.6}", self.acc)
            };
            self.history = format!("{} {}", acc_str, op);
            self.display = acc_str;
            self.pending_op = Some(op);
            self.clear_on_next = true;
        }
    }

    pub fn calculate_equals(&mut self) {
        if let (Some(op), Ok(val)) = (self.pending_op, self.display.parse::<f64>()) {
            let res = match op {
                '+' => self.acc + val,
                '-' => self.acc - val,
                '*' => self.acc * val,
                '/' => if val != 0.0 { self.acc / val } else { 0.0 },
                '^' => self.acc.powf(val),
                _ => val,
            };
            self.history = format!("{} {} {} =", self.acc as i64, op, val as i64);
            if res.fract() == 0.0 && res.abs() < 1e15 {
                self.display = format!("{}", res as i64);
            } else {
                self.display = format!("{:.6}", res);
            }
            self.prog_val = (res as i64) as u64;
            self.pending_op = None;
            self.clear_on_next = true;
        }
    }

    pub fn clear_all(&mut self) {
        self.display = String::from("0");
        self.history.clear();
        self.acc = 0.0;
        self.pending_op = None;
        self.clear_on_next = false;
        self.prog_val = 0;
        self.prog_acc = 0;
        self.prog_op = None;
        self.prog_clear_on_next = false;
    }

    pub fn clear_entry(&mut self) {
        self.display = String::from("0");
        self.clear_on_next = false;
    }

    pub fn backspace(&mut self) {
        if !self.clear_on_next && self.display.len() > 1 {
            self.display.pop();
        } else {
            self.display = String::from("0");
        }
    }

    pub fn scientific_func(&mut self, func: &str) {
        if let Ok(v) = self.display.parse::<f64>() {
            let res = match func {
                "sin" => v.sin(),
                "cos" => v.cos(),
                "tan" => v.tan(),
                "log" => if v > 0.0 { v.log10() } else { 0.0 },
                "ln" => if v > 0.0 { v.ln() } else { 0.0 },
                "sqrt" => if v >= 0.0 { v.sqrt() } else { 0.0 },
                "inv" => if v != 0.0 { 1.0 / v } else { 0.0 },
                "fact" | "n!" => {
                    let mut prod = 1.0;
                    let n = (v as i64).clamp(0, 20);
                    for i in 2..=n {
                        prod *= i as f64;
                    }
                    prod
                }
                "neg" => -v,
                _ => v,
            };
            self.history = format!("{}({})", func, v as i64);
            if res.fract() == 0.0 && res.abs() < 1e15 {
                self.display = format!("{}", res as i64);
            } else {
                self.display = format!("{:.6}", res);
            }
            self.prog_val = (res as i64) as u64;
            self.clear_on_next = true;
        }
    }

    pub fn programmer_mask(&self) -> u64 {
        match self.prog_word_size {
            0 => 0xFFFF_FFFF_FFFF_FFFF, // QWORD
            1 => 0x0000_0000_FFFF_FFFF, // DWORD
            2 => 0x0000_0000_0000_FFFF, // WORD
            3 => 0x0000_0000_0000_00FF, // BYTE
            _ => 0xFFFF_FFFF_FFFF_FFFF,
        }
    }

    pub fn prog_apply_mask(&mut self) {
        self.prog_val &= self.programmer_mask();
        self.display = format!("{}", self.prog_val);
    }

    pub fn prog_input_hex(&mut self, nibble: u64) {
        if self.prog_clear_on_next {
            self.prog_val = 0;
            self.prog_clear_on_next = false;
        }
        self.prog_val = self.prog_val.wrapping_shl(4) | (nibble & 0xF);
        self.prog_apply_mask();
    }

    fn prog_eval(lhs: u64, rhs: u64, op: char) -> u64 {
        match op {
            '&' => lhs & rhs,
            '|' => lhs | rhs,
            '^' => lhs ^ rhs,
            '%' => if rhs != 0 { lhs % rhs } else { 0 },
            _ => rhs,
        }
    }

    pub fn prog_set_op(&mut self, op: char) {
        let mask = self.programmer_mask();
        let cur = self.prog_val & mask;
        if let Some(prev_op) = self.prog_op {
            if !self.prog_clear_on_next {
                self.prog_acc = Self::prog_eval(self.prog_acc & mask, cur, prev_op) & mask;
                self.prog_val = self.prog_acc;
                self.prog_apply_mask();
            }
        } else {
            self.prog_acc = cur;
        }
        self.prog_op = Some(op);
        self.prog_clear_on_next = true;
    }

    pub fn prog_equals(&mut self) {
        if let Some(op) = self.prog_op.take() {
            let mask = self.programmer_mask();
            let lhs = self.prog_acc & mask;
            let rhs = self.prog_val & mask;
            self.prog_val = Self::prog_eval(lhs, rhs, op) & mask;
            self.prog_acc = self.prog_val;
            self.prog_apply_mask();
            self.prog_clear_on_next = true;
        }
    }

    pub fn prog_clear(&mut self) {
        self.prog_val = 0;
        self.prog_acc = 0;
        self.prog_op = None;
        self.prog_clear_on_next = false;
        self.display = String::from("0");
    }
}

#[derive(Clone)]
pub struct StickyNote {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub color_idx: usize, // 0: Amarelo, 1: Azul, 2: Rosa, 3: Verde
    pub text: String,
}

pub struct StickyNotesState {
    pub notes: Vec<StickyNote>,
    pub active_idx: usize,
    pub next_id: usize,
    pub status_msg: String,
    pub last_canvas: (usize, usize, usize, usize),
}

impl StickyNotesState {
    pub fn new() -> Self {
        let mut notes = Vec::new();
        notes.push(StickyNote {
            id: 1,
            x: 20,
            y: 50,
            w: 220,
            h: 180,
            color_idx: 0,
            text: String::from("Planejamento HybridOS:\n- Integrar scheduler Ring 3\n- Testar driver de rede E1000\n- Finalizar Suite Office"),
        });
        notes.push(StickyNote {
            id: 2,
            x: 260,
            y: 70,
            w: 220,
            h: 180,
            color_idx: 1,
            text: String::from("Lembrete VFS:\n- Persistencia garantida em /etc/stickies.txt\n- Suporte a caminhos C:\\ e / transparentes"),
        });
        Self {
            notes,
            active_idx: 0,
            next_id: 3,
            status_msg: String::from("Sincronizado com /etc/stickies.txt"),
            last_canvas: (0, 0, 0, 0),
        }
    }

    pub fn add_note(&mut self) {
        let offset = (self.notes.len() * 30) % 150;
        let c_idx = self.notes.len() % 4;
        let note = StickyNote {
            id: self.next_id,
            x: 40 + offset,
            y: 60 + offset,
            w: 220,
            h: 180,
            color_idx: c_idx,
            text: format!("Nova Nota #{}:\nClique aqui para digitar suas anotacoes...", self.next_id),
        };
        self.next_id += 1;
        self.notes.push(note);
        self.active_idx = self.notes.len() - 1;
        self.save_to_vfs();
    }

    pub fn delete_active(&mut self) {
        if !self.notes.is_empty() {
            self.notes.remove(self.active_idx);
            if self.active_idx >= self.notes.len() && !self.notes.is_empty() {
                self.active_idx = self.notes.len() - 1;
            }
            self.save_to_vfs();
        }
    }

    pub fn cycle_color(&mut self) {
        if let Some(note) = self.notes.get_mut(self.active_idx) {
            note.color_idx = (note.color_idx + 1) % 4;
            self.save_to_vfs();
        }
    }

    pub fn save_to_vfs(&mut self) {
        let mut data = String::new();
        for n in &self.notes {
            data.push_str(&format!("[NOTE:id={},color={},x={},y={},w={},h={}]\n{}\n[END_NOTE]\n", n.id, n.color_idx, n.x, n.y, n.w, n.h, n.text));
        }
        let _ = vfs::vfs_unlink("/etc/stickies.txt");
        let _ = vfs::vfs_create_file("/etc/stickies.txt", data.as_bytes(), Permissions::default_file());
        self.status_msg = String::from("Salvo em /etc/stickies.txt");
    }

    pub fn load_from_vfs(&mut self) {
        if let Ok(bytes) = vfs::vfs_read_all("/etc/stickies.txt") {
            if let Ok(content) = core::str::from_utf8(&bytes) {
                if !content.trim().is_empty() {
                    let mut parsed = Vec::new();
                    let parts: Vec<&str> = content.split("[END_NOTE]").collect();
                    for part in parts {
                        let trimmed = part.trim();
                        if trimmed.is_empty() { continue; }
                        if let Some(header_end) = trimmed.find("]\n") {
                            let header = &trimmed[..header_end];
                            let body = &trimmed[header_end + 2..];
                            let mut c_idx = 0;
                            if let Some(pos) = header.find("color=") {
                                if let Some(digit) = header[pos + 6..].chars().next() {
                                    c_idx = digit.to_digit(10).unwrap_or(0) as usize;
                                }
                            }
                            parsed.push(StickyNote {
                                id: parsed.len() + 1,
                                x: 30 + (parsed.len() * 40) % 200,
                                y: 50 + (parsed.len() * 30) % 150,
                                w: 220,
                                h: 180,
                                color_idx: c_idx,
                                text: String::from(body),
                            });
                        }
                    }
                    if !parsed.is_empty() {
                        self.notes = parsed;
                        self.active_idx = 0;
                        self.status_msg = String::from("Carregado de /etc/stickies.txt");
                    }
                }
            }
        }
    }
}

pub struct CalendarClockState {
    pub year: i32,
    pub month: u32,
    pub selected_day: u32,
    // Stopwatch (100Hz PIT / SYSTEM_TICKS)
    pub sw_running: bool,
    pub sw_start_tick: u64,
    pub sw_accum_ticks: u64,
    pub laps: Vec<u64>,
    pub last_canvas: (usize, usize, usize, usize),
}

impl CalendarClockState {
    pub fn new() -> Self {
        Self {
            year: 2026,
            month: 11,
            selected_day: 15,
            sw_running: false,
            sw_start_tick: 0,
            sw_accum_ticks: 0,
            laps: Vec::new(),
            last_canvas: (0, 0, 0, 0),
        }
    }

    pub fn current_sw_ticks(&self) -> u64 {
        if self.sw_running {
            let cur = SYSTEM_TICKS.load(Ordering::Relaxed);
            self.sw_accum_ticks + (cur.saturating_sub(self.sw_start_tick))
        } else {
            self.sw_accum_ticks
        }
    }

    pub fn toggle_stopwatch(&mut self) {
        if self.sw_running {
            let cur = SYSTEM_TICKS.load(Ordering::Relaxed);
            self.sw_accum_ticks += cur.saturating_sub(self.sw_start_tick);
            self.sw_running = false;
        } else {
            self.sw_start_tick = SYSTEM_TICKS.load(Ordering::Relaxed);
            self.sw_running = true;
        }
    }

    pub fn reset_stopwatch(&mut self) {
        self.sw_running = false;
        self.sw_start_tick = 0;
        self.sw_accum_ticks = 0;
        self.laps.clear();
    }

    pub fn record_lap(&mut self) {
        if self.laps.len() < 8 {
            let t = self.current_sw_ticks();
            self.laps.push(t);
        }
    }

    pub fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
    }

    pub fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
    }
}

pub struct CharMapState {
    pub selected_char: u8,
    pub status_msg: String,
    pub last_canvas: (usize, usize, usize, usize),
}

impl CharMapState {
    pub fn new() -> Self {
        Self {
            selected_char: 65, // 'A'
            status_msg: String::from("Clique em um caractere para visualizar"),
            last_canvas: (0, 0, 0, 0),
        }
    }
}

pub struct AddressBookState {
    pub contacts: Vec<ContactItem>,
    pub selected_idx: usize,
    pub search_query: String,
    pub edit_field: usize, // 0: Nome, 1: Email, 2: Tel, 3: End, 4: Empresa, 5: Busca
    pub status_msg: String,
    pub last_canvas: (usize, usize, usize, usize),
}

#[derive(Clone)]
pub struct ContactItem {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub company: String,
}

impl AddressBookState {
    pub fn new() -> Self {
        let mut contacts = Vec::new();
        contacts.push(ContactItem {
            name: String::from("Linus Torvalds"),
            email: String::from("linus@kernel.org"),
            phone: String::from("+1 503 555-0100"),
            address: String::from("Portland, Oregon"),
            company: String::from("Linux Foundation"),
        });
        contacts.push(ContactItem {
            name: String::from("Dave Cutler"),
            email: String::from("dcutler@microsoft.com"),
            phone: String::from("+1 425 555-0199"),
            address: String::from("Redmond, Washington"),
            company: String::from("Microsoft Windows NT"),
        });
        contacts.push(ContactItem {
            name: String::from("Vinicius Dev"),
            email: String::from("vinicius@hybridos.org"),
            phone: String::from("+55 11 99999-8888"),
            address: String::from("Sao Paulo, Brasil"),
            company: String::from("HybridOS Core Team"),
        });
        Self {
            contacts,
            selected_idx: 0,
            search_query: String::new(),
            edit_field: 0,
            status_msg: String::from("3 contatos carregados"),
            last_canvas: (0, 0, 0, 0),
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search_query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return (0..self.contacts.len()).collect();
        }
        self.contacts
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.name.to_ascii_lowercase().contains(&q)
                    || c.email.to_ascii_lowercase().contains(&q)
                    || c.phone.to_ascii_lowercase().contains(&q)
                    || c.address.to_ascii_lowercase().contains(&q)
                    || c.company.to_ascii_lowercase().contains(&q)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub fn export_csv(&mut self) {
        let mut csv = String::from("Nome,Email,Telefone,Endereco,Empresa\n");
        for c in &self.contacts {
            csv.push_str(&format!("{},{},{},{},{}\n", c.name, c.email, c.phone, c.address, c.company));
        }
        let _ = vfs::vfs_unlink("/contatos.csv");
        let res = vfs::vfs_create_file("/contatos.csv", csv.as_bytes(), Permissions::default_file());
        match res {
            Ok(_) => {
                self.status_msg = format!("Exportado com sucesso para C:\\contatos.csv ({} itens)", self.contacts.len());
            }
            Err(_) => {
                self.status_msg = String::from("Erro ao exportar C:\\contatos.csv");
            }
        }
    }
}

// Master Category 5 Container
pub struct Cat5SuiteState {
    pub initialized: bool,
    pub notepad: NotepadState,
    pub wordpad: WordPadState,
    pub calc: CalculatorState,
    pub stickies: StickyNotesState,
    pub calendar: CalendarClockState,
    pub charmap: CharMapState,
    pub contacts: AddressBookState,
    pub clipboard_canvas: (usize, usize, usize, usize),
    pub stickies_loaded: bool,
}

impl Cat5SuiteState {
    pub fn new() -> Self {
        Self {
            initialized: true,
            notepad: NotepadState::new(),
            wordpad: WordPadState::new(),
            calc: CalculatorState::new(),
            stickies: StickyNotesState::new(),
            calendar: CalendarClockState::new(),
            charmap: CharMapState::new(),
            contacts: AddressBookState::new(),
            clipboard_canvas: (0, 0, 0, 0),
            stickies_loaded: false,
        }
    }
}

pub static CAT5_STATE: SpinLock<Cat5SuiteState> = SpinLock::new(Cat5SuiteState {
    initialized: false,
    notepad: NotepadState {
        text: String::new(),
        cursor_pos: 0,
        search_query: String::new(),
        search_open: false,
        word_wrap: false,
        status_msg: String::new(),
        last_canvas: (0, 0, 0, 0),
    },
    wordpad: WordPadState {
        text: String::new(),
        cursor_pos: 0,
        bold: false,
        italic: false,
        underline: false,
        color_idx: 0,
        align_mode: 0,
        status_msg: String::new(),
        last_canvas: (0, 0, 0, 0),
    },
    calc: CalculatorState {
        mode: 0,
        display: String::new(),
        history: String::new(),
        acc: 0.0,
        pending_op: None,
        clear_on_next: false,
        prog_val: 0,
        prog_acc: 0,
        prog_op: None,
        prog_clear_on_next: false,
        prog_word_size: 0,
        last_canvas: (0, 0, 0, 0),
    },
    stickies: StickyNotesState {
        notes: Vec::new(),
        active_idx: 0,
        next_id: 1,
        status_msg: String::new(),
        last_canvas: (0, 0, 0, 0),
    },
    calendar: CalendarClockState {
        year: 2026,
        month: 11,
        selected_day: 15,
        sw_running: false,
        sw_start_tick: 0,
        sw_accum_ticks: 0,
        laps: Vec::new(),
        last_canvas: (0, 0, 0, 0),
    },
    charmap: CharMapState {
        selected_char: 65,
        status_msg: String::new(),
        last_canvas: (0, 0, 0, 0),
    },
    contacts: AddressBookState {
        contacts: Vec::new(),
        selected_idx: 0,
        search_query: String::new(),
        edit_field: 0,
        status_msg: String::new(),
        last_canvas: (0, 0, 0, 0),
    },
    clipboard_canvas: (0, 0, 0, 0),
    stickies_loaded: false,
});

pub fn ensure_cat5_init() {
    let mut state = CAT5_STATE.lock();
    if !state.initialized {
        state.notepad = NotepadState::new();
        state.wordpad = WordPadState::new();
        state.calc = CalculatorState::new();
        state.stickies = StickyNotesState::new();
        state.calendar = CalendarClockState::new();
        state.charmap = CharMapState::new();
        state.contacts = AddressBookState::new();
        state.clipboard_canvas = (0, 0, 0, 0);
        state.initialized = true;

        // Populate initial clipboard examples once at initialization
        let mut clip = GLOBAL_CLIPBOARD.lock();
        if clip.items.is_empty() {
            clip.push(String::from("C:\\Users\\Vinicius\\Documents\\HybridOS-Kernel"));
            clip.push(String::from("hybrid_kernel_main(boot_info: &'static LimineBootInfo)"));
            clip.push(String::from("/etc/stickies.txt"));
            clip.push(String::from("0x0000_7FFF_FFFF_0000"));
            clip.push(String::from("engenharia@hybridos.org"));
        }
    }
    if !state.stickies_loaded {
        state.stickies.load_from_vfs();
        state.stickies_loaded = true;
    }
}

// ============================================================================
// CMOS RTC Reader
// ============================================================================

pub struct RtcTime {
    pub sec: u8,
    pub min: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

unsafe fn cmos_read(reg: u8) -> u8 {
    outb(0x70, reg);
    inb(0x71)
}

fn bcd_to_bin(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}

pub fn read_rtc() -> RtcTime {
    unsafe {
        let mut timeout = 10000;
        while (cmos_read(0x0A) & 0x80) != 0 && timeout > 0 {
            timeout -= 1;
        }
        let mut sec = cmos_read(0x00);
        let mut min = cmos_read(0x02);
        let mut hour = cmos_read(0x04);
        let mut day = cmos_read(0x07);
        let mut month = cmos_read(0x08);
        let mut year_val = cmos_read(0x09) as u16;

        let status_b = cmos_read(0x0B);
        if (status_b & 0x04) == 0 {
            sec = bcd_to_bin(sec);
            min = bcd_to_bin(min);
            hour = ((hour & 0x7F) >> 4) * 10 + (hour & 0x0F) | (hour & 0x80);
            day = bcd_to_bin(day);
            month = bcd_to_bin(month);
            year_val = bcd_to_bin(year_val as u8) as u16;
        }
        if (status_b & 0x02) == 0 && (hour & 0x80) != 0 {
            hour = ((hour & 0x7F) + 12) % 24;
        }

        RtcTime {
            sec: sec.min(59),
            min: min.min(59),
            hour: hour.min(23),
            day: day.clamp(1, 31),
            month: month.clamp(1, 12),
            year: 2000 + year_val,
        }
    }
}

// Day of week calculation (0: Domingo, 1: Segunda ... 6: Sábado)
pub fn calc_day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = (month.clamp(1, 12) - 1) as usize;
    let dow = (y + y / 4 - y / 100 + y / 400 + t[m] + day as i32) % 7;
    if dow < 0 { (dow + 7) as u32 } else { dow as u32 }
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

// Bresenham's line algorithm for analog clock hands
unsafe fn draw_line(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, x0: isize, y0: isize, x1: isize, y1: isize, color: u32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;

    loop {
        if cx >= 0 && (cx as usize) < scr_w && cy >= 0 && (cy as usize) < scr_h {
            let ptr = fb.add((cy as usize) * pitch) as *mut u32;
            *ptr.add(cx as usize) = color;
        }
        if cx == x1 && cy == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

// ============================================================================
// Public Entry Points Required by Kernel & Compositor
// ============================================================================

/// Render Category 5 application content inside client canvas (cx, cy, cw, ch).
pub unsafe fn render_cat5_app(
    id: u8,
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    desk: &mut DesktopState,
) {
    ensure_cat5_init();

    match id {
        34 | 0 => render_notepad(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        35 | 1 => render_wordpad(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        36 | 2 => render_calculator(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        37 | 3 => render_sticky_notes(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        38 | 4 => render_calendar_clock(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        39 | 5 => render_charmap(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        40 | 6 => render_clipboard_mgr(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        41 | 7 => render_address_book(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, desk),
        _ => {}
    }
}

/// Handle mouse clicks for Category 5 applications.
pub fn handle_cat5_click(id: u8, cx: usize, cy: usize, desk: &mut DesktopState) -> bool {
    ensure_cat5_init();
    let mut state = CAT5_STATE.lock();

    // Normalize coordinates (handle screen vs client canvas coordinates)
    match id {
        34 | 0 => {
            let (lx, ly) = to_local_coords(cx, cy, state.notepad.last_canvas);
            handle_notepad_click(&mut state.notepad, lx, ly, desk)
        }
        35 | 1 => {
            let (lx, ly) = to_local_coords(cx, cy, state.wordpad.last_canvas);
            handle_wordpad_click(&mut state.wordpad, lx, ly, desk)
        }
        36 | 2 => {
            let (lx, ly) = to_local_coords(cx, cy, state.calc.last_canvas);
            handle_calc_click(&mut state.calc, lx, ly, desk)
        }
        37 | 3 => {
            let (lx, ly) = to_local_coords(cx, cy, state.stickies.last_canvas);
            handle_stickies_click(&mut state.stickies, lx, ly, desk)
        }
        38 | 4 => {
            let (lx, ly) = to_local_coords(cx, cy, state.calendar.last_canvas);
            handle_calendar_clock_click(&mut state.calendar, lx, ly, desk)
        }
        39 | 5 => {
            let (lx, ly) = to_local_coords(cx, cy, state.charmap.last_canvas);
            handle_charmap_click(&mut state.charmap, lx, ly, desk)
        }
        40 | 6 => {
            let canvas = state.clipboard_canvas;
            let (lx, ly) = to_local_coords(cx, cy, canvas);
            handle_clipboard_click(lx, ly, canvas.2, desk)
        }
        41 | 7 => {
            let (lx, ly) = to_local_coords(cx, cy, state.contacts.last_canvas);
            handle_contacts_click(&mut state.contacts, lx, ly, desk)
        }
        _ => false,
    }
}

/// Handle keyboard character inputs for Category 5 applications.
pub fn handle_cat5_key(id: u8, ch: char, desk: &mut DesktopState) -> bool {
    ensure_cat5_init();
    let mut state = CAT5_STATE.lock();

    match id {
        34 | 0 => {
            if state.notepad.search_open {
                match ch {
                    '\x08' => {
                        state.notepad.search_query.pop();
                    }
                    '\x1b' => {
                        state.notepad.search_open = false;
                    }
                    '\r' | '\n' => {
                        let q = state.notepad.search_query.to_ascii_lowercase();
                        if !q.is_empty() {
                            let lower = state.notepad.text.to_ascii_lowercase();
                            let count = lower.matches(&q).count();
                            let start = state.notepad.cursor_pos.min(lower.len());
                            let found = lower[start..]
                                .find(&q)
                                .map(|off| start + off)
                                .or_else(|| lower.find(&q));
                            if let Some(pos) = found {
                                state.notepad.cursor_pos = (pos + q.len()).min(state.notepad.text.len());
                                state.notepad.status_msg = format!("Localizado '{}' ({} ocorr.)", state.notepad.search_query, count);
                            } else {
                                state.notepad.status_msg = format!("Termo '{}' nao encontrado", state.notepad.search_query);
                            }
                        }
                    }
                    c if !c.is_control() => {
                        if state.notepad.search_query.len() < 32 {
                            state.notepad.search_query.push(c);
                        }
                    }
                    _ => {}
                }
            } else {
                match ch {
                    '\x08' => state.notepad.backspace(),
                    '\r' | '\n' => state.notepad.insert_char('\n'),
                    '\t' => {
                        for _ in 0..4 { state.notepad.insert_char(' '); }
                    }
                    c if !c.is_control() => state.notepad.insert_char(c),
                    _ => {}
                }
            }
            desk.needs_full_redraw = true;
            true
        }
        35 | 1 => {
            let wp = &mut state.wordpad;
            match ch {
                '\x08' => {
                    if wp.cursor_pos > 0 && !wp.text.is_empty() {
                        let prev = wp.text[..wp.cursor_pos].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                        wp.text.remove(prev);
                        wp.cursor_pos = prev;
                    }
                }
                '\r' | '\n' => {
                    let cpos = wp.cursor_pos;
                    wp.text.insert(cpos, '\n');
                    wp.cursor_pos += 1;
                }
                c if !c.is_control() => {
                    let cpos = wp.cursor_pos;
                    wp.text.insert(cpos, c);
                    wp.cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            desk.needs_full_redraw = true;
            true
        }
        36 | 2 => {
            if state.calc.mode == 2 {
                match ch {
                    '0'..='9' => state.calc.prog_input_hex((ch as u8 - b'0') as u64),
                    'a'..='f' => state.calc.prog_input_hex((ch as u8 - b'a' + 10) as u64),
                    'A'..='F' => state.calc.prog_input_hex((ch as u8 - b'A' + 10) as u64),
                    '&' | '|' | '^' | '%' => state.calc.prog_set_op(ch),
                    '=' | '\r' | '\n' => state.calc.prog_equals(),
                    '\x08' => {
                        state.calc.prog_val >>= 4;
                        state.calc.prog_apply_mask();
                    }
                    '\x1b' => state.calc.prog_clear(),
                    _ => {}
                }
            } else {
                match ch {
                    '0'..='9' => state.calc.input_digit(ch),
                    '.' | ',' => state.calc.input_dot(),
                    '+' | '-' | '*' | '/' | '^' => state.calc.set_op(ch),
                    '=' | '\r' | '\n' => state.calc.calculate_equals(),
                    '\x08' => state.calc.backspace(),
                    'c' | 'C' => state.calc.clear_all(),
                    _ => {}
                }
            }
            desk.needs_full_redraw = true;
            true
        }
        37 | 3 => {
            let act_idx = state.stickies.active_idx;
            if let Some(note) = state.stickies.notes.get_mut(act_idx) {
                match ch {
                    '\x08' => {
                        if !note.text.is_empty() {
                            note.text.pop();
                        }
                    }
                    '\r' | '\n' => note.text.push('\n'),
                    c if !c.is_control() => note.text.push(c),
                    _ => {}
                }
            }
            state.stickies.save_to_vfs();
            desk.needs_full_redraw = true;
            true
        }
        41 | 7 => {
            let field = state.contacts.edit_field;
            if field == 5 {
                match ch {
                    '\x08' => {
                        state.contacts.search_query.pop();
                    }
                    '\r' | '\n' => {
                        let filtered = state.contacts.filtered_indices();
                        if let Some(&first_idx) = filtered.first() {
                            state.contacts.selected_idx = first_idx;
                        }
                        state.contacts.edit_field = 0;
                    }
                    '\x1b' => {
                        state.contacts.search_query.clear();
                        state.contacts.edit_field = 0;
                    }
                    c if !c.is_control() => {
                        if state.contacts.search_query.len() < 24 {
                            state.contacts.search_query.push(c);
                        }
                    }
                    _ => {}
                }
                let filtered = state.contacts.filtered_indices();
                if !filtered.is_empty() && !filtered.contains(&state.contacts.selected_idx) {
                    state.contacts.selected_idx = filtered[0];
                }
                state.contacts.status_msg = if state.contacts.search_query.is_empty() {
                    format!("{} contatos carregados", state.contacts.contacts.len())
                } else {
                    format!("Filtro '{}': {} encontrado(s)", state.contacts.search_query, filtered.len())
                };
                desk.needs_full_redraw = true;
                return true;
            }
            let sel = state.contacts.selected_idx;
            let mut do_advance_field = false;
            if let Some(contact) = state.contacts.contacts.get_mut(sel) {
                let target = match field {
                    0 => &mut contact.name,
                    1 => &mut contact.email,
                    2 => &mut contact.phone,
                    3 => &mut contact.address,
                    4 => &mut contact.company,
                    _ => &mut contact.name,
                };
                match ch {
                    '\x08' => { target.pop(); }
                    '\r' | '\n' => {
                        do_advance_field = true;
                    }
                    c if !c.is_control() => { target.push(c); }
                    _ => {}
                }
            }
            if do_advance_field {
                state.contacts.edit_field = (field + 1) % 5;
            }
            desk.needs_full_redraw = true;
            true
        }
        _ => false,
    }
}

fn to_local_coords(cx: usize, cy: usize, last_canvas: (usize, usize, usize, usize)) -> (usize, usize) {
    let (bx, by, bw, bh) = last_canvas;
    if cx >= bx && cx < bx + bw && cy >= by && cy < by + bh {
        (cx - bx, cy - by)
    } else {
        (cx, cy)
    }
}

// ============================================================================
// 34. Bloco de Notas (Notepad)
// ============================================================================

unsafe fn render_notepad(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.notepad.last_canvas = (cx, cy, cw, ch);

    // 1. Menu Strip (24px)
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, 24, 0, 0x00F1F5F9);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 12, cy + 4, "Arquivo", 0x000F172A, None);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 80, cy + 4, "Editar", 0x000F172A, None);
    let fmt_col = if state.notepad.word_wrap { 0x000284C7 } else { 0x000F172A };
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 140, cy + 4, "Formatar", fmt_col, None);

    // 2. Toolbar Strip (30px)
    let tb_y = cy + 24;
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, tb_y, cw, 30);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 8, tb_y + 3, 56, 24, "Novo", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 70, tb_y + 3, 105, 24, "Abrir VFS", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 181, tb_y + 3, 115, 24, "Salvar em C:\\", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 302, tb_y + 3, 85, 24, "Localizar", state.notepad.search_open);

    // Search Box if active
    let query_lower = state.notepad.search_query.to_ascii_lowercase();
    let total_matches = if state.notepad.search_open && !query_lower.is_empty() {
        state.notepad.text.to_ascii_lowercase().matches(&query_lower).count()
    } else {
        0
    };
    let mut text_y_start = tb_y + 30;
    if state.notepad.search_open {
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, text_y_start, cw, 28, 0, 0x00E0F2FE);
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 10, text_y_start + 6, "Buscar:", 0x000369A1, None);
        draw_glass_textbox(fb, pitch, scr_w, scr_h, cx + 70, text_y_start + 3, 200, 22, &state.notepad.search_query, true);
        draw_glass_button(fb, pitch, scr_w, scr_h, cx + 276, text_y_start + 3, 40, 22, "X", false);
        if !query_lower.is_empty() {
            let m_str = format!("{} ocorr.", total_matches);
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 324, text_y_start + 6, &m_str, 0x000369A1, None);
        }
        text_y_start += 28;
    }

    // 3. Multi-line Text Area
    let status_h = 22usize;
    let edit_h = (cy + ch).saturating_sub(text_y_start + status_h);
    let gutter_w = 42usize;

    // Line numbering gutter
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, text_y_start, gutter_w, edit_h, 0, 0x00F8FAFC);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + gutter_w, text_y_start, cw.saturating_sub(gutter_w), edit_h, 0, 0x00FFFFFF);

    let max_cols = ((cw.saturating_sub(gutter_w + 16)) / 8).max(1);
    let max_rows = edit_h / 18;

    let mut line_num = 1usize;
    let mut draw_row = 0usize;
    let tick = SYSTEM_TICKS.load(Ordering::Relaxed);
    let show_cursor = (tick / 50) % 2 == 0;

    let (cur_line, cur_col) = state.notepad.line_col();
    let do_highlight = state.notepad.search_open && !query_lower.is_empty();
    let q_char_len = state.notepad.search_query.chars().count();

    if state.notepad.text.is_empty() {
        let gy = text_y_start + 6;
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 6, gy, "  1", 0x0094A3B8, None);
        if show_cursor && !state.notepad.search_open {
            let cursor_px = cx + gutter_w + 8;
            for cy_p in gy..(gy + 16) {
                if cy_p < scr_h && cursor_px < scr_w {
                    let ptr = fb.add(cy_p * pitch) as *mut u32;
                    *ptr.add(cursor_px) = 0x000284C7;
                }
            }
        }
    } else {
        for raw_line in state.notepad.text.split('\n') {
            if draw_row >= max_rows { break; }

            let mut segments: Vec<String> = Vec::new();
            if state.notepad.word_wrap && raw_line.chars().count() > max_cols {
                let mut cur_seg = String::new();
                for ch in raw_line.chars() {
                    cur_seg.push(ch);
                    if cur_seg.chars().count() >= max_cols {
                        segments.push(cur_seg);
                        cur_seg = String::new();
                    }
                }
                if !cur_seg.is_empty() {
                    segments.push(cur_seg);
                }
            } else {
                let display_line: String = raw_line.chars().take(max_cols).collect();
                segments.push(display_line);
            }

            let mut col_offset = 0usize;
            for (seg_idx, display_line) in segments.iter().enumerate() {
                if draw_row >= max_rows { break; }
                let gy = text_y_start + 6 + draw_row * 18;

                // Gutter line number on first visual segment of logical line
                if seg_idx == 0 {
                    let num_str = format!("{:>3}", line_num);
                    draw_text_px(fb, pitch, scr_w, scr_h, cx + 6, gy, &num_str, 0x0094A3B8, None);
                }

                // Highlight occurrences of search_query on this line
                if do_highlight {
                    let seg_lower = display_line.to_ascii_lowercase();
                    let mut search_from = 0usize;
                    while search_from < seg_lower.len() {
                        if let Some(rel) = seg_lower[search_from..].find(&query_lower) {
                            let match_byte = search_from + rel;
                            let match_col = seg_lower[..match_byte].chars().count();
                            let hx = cx + gutter_w + 8 + match_col * 8;
                            let hw = (q_char_len * 8).min(cw.saturating_sub(gutter_w + 16 + match_col * 8));
                            if hw > 0 {
                                fill_rounded_rect(fb, pitch, scr_w, scr_h, hx, gy, hw, 16, 2, 0x00FEF08A);
                            }
                            search_from = match_byte + query_lower.len().max(1);
                        } else {
                            break;
                        }
                    }
                }

                // Content
                draw_text_px(fb, pitch, scr_w, scr_h, cx + gutter_w + 8, gy, display_line, 0x000F172A, None);

                // Draw blinking cursor if on current line & segment
                let seg_chars = display_line.chars().count();
                let cur_zero = cur_col.saturating_sub(1);
                let is_last_seg = seg_idx + 1 == segments.len();
                let cursor_in_seg = cur_zero >= col_offset && (cur_zero < col_offset + seg_chars || is_last_seg);
                if show_cursor && !state.notepad.search_open && line_num == cur_line && cursor_in_seg {
                    let local_col = cur_zero.saturating_sub(col_offset);
                    let cursor_px = cx + gutter_w + 8 + (local_col * 8).min(cw.saturating_sub(gutter_w + 16));
                    for cy_p in gy..(gy + 16) {
                        if cy_p < scr_h && cursor_px < scr_w {
                            let ptr = fb.add(cy_p * pitch) as *mut u32;
                            *ptr.add(cursor_px) = 0x000284C7;
                        }
                    }
                }

                col_offset += seg_chars;
                draw_row += 1;
            }

            line_num += 1;
        }
    }

    // 4. Status Bar
    let status_y = cy + ch - status_h;
    let wrap_lbl = if state.notepad.word_wrap { "Wrap: ON" } else { "Wrap: OFF" };
    let status_left = format!("Lin {}, Col {} | {} chars | UTF-8 | {}", cur_line, cur_col, state.notepad.text.len(), wrap_lbl);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, status_h, &status_left, &state.notepad.status_msg);
}

fn handle_notepad_click(notepad: &mut NotepadState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // 1. Menu Strip (y: 0..24)
    if ly < 24 {
        if lx >= 8 && lx < 74 {
            // "Arquivo": abrir se vazio, salvar caso contrario
            if notepad.text.trim().is_empty() {
                notepad.load_from_vfs();
            } else {
                notepad.save_to_vfs();
            }
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 74 && lx < 134 {
            // "Editar": colar do GLOBAL_CLIPBOARD
            let pasted = GLOBAL_CLIPBOARD.lock().get_active().cloned();
            if let Some(clip_text) = pasted {
                for c in clip_text.chars() {
                    notepad.insert_char(c);
                }
                notepad.status_msg = String::from("Colado da area de transferencia");
            } else {
                notepad.status_msg = String::from("Area de transferencia vazia");
            }
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 134 && lx < 220 {
            // "Formatar": alternar quebra de linha automatica
            notepad.word_wrap = !notepad.word_wrap;
            notepad.status_msg = if notepad.word_wrap {
                String::from("Quebra automatica de linha ativada")
            } else {
                String::from("Quebra automatica de linha desativada")
            };
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // 2. Toolbar buttons (y: 24..54)
    if ly >= 24 && ly <= 54 {
        if lx >= 8 && lx <= 64 {
            // Novo
            notepad.text.clear();
            notepad.cursor_pos = 0;
            notepad.status_msg = String::from("Novo documento");
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 70 && lx <= 175 {
            // Abrir do VFS
            notepad.load_from_vfs();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 181 && lx <= 296 {
            // Salvar em C:\
            notepad.save_to_vfs();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 302 && lx <= 387 {
            // Localizar
            notepad.search_open = !notepad.search_open;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Close search box
    if notepad.search_open && ly >= 54 && ly <= 82 && lx >= 276 && lx <= 316 {
        notepad.search_open = false;
        desk.needs_full_redraw = true;
        return true;
    }

    // Click inside text area to position cursor
    let text_start_y = if notepad.search_open { 82 } else { 54 };
    if ly >= text_start_y {
        let row_idx = (ly - text_start_y) / 18;
        let mut current_row = 0;
        let mut char_count = 0;
        for line in notepad.text.split('\n') {
            if current_row == row_idx {
                let col_idx = lx.saturating_sub(50) / 8;
                let byte_off = line
                    .char_indices()
                    .nth(col_idx)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
                notepad.cursor_pos = (char_count + byte_off).min(notepad.text.len());
                desk.needs_full_redraw = true;
                return true;
            }
            char_count += line.len() + 1; // + newline
            current_row += 1;
        }
        notepad.cursor_pos = notepad.text.len();
        desk.needs_full_redraw = true;
        return true;
    }

    false
}

// ============================================================================
// 35. Editor de Documentos Formatados (WordPad)
// ============================================================================

unsafe fn draw_styled_text_px(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    x: usize, y: usize, text: &str, color: u32,
    bold: bool, italic: bool,
) {
    if !italic {
        draw_text_px(fb, pitch, scr_w, scr_h, x, y, text, color, None);
        if bold {
            draw_text_px(fb, pitch, scr_w, scr_h, x + 1, y, text, color, None);
        }
        return;
    }

    let mut cur_x = x;
    for ch in text.chars() {
        let code = ch as usize;
        let glyph_idx = if (32..128).contains(&code) {
            code - 32
        } else {
            b'?' as usize - 32
        };
        let font_offset = glyph_idx * 16;
        for row in 0..16usize {
            let py = y + row;
            if py >= scr_h {
                break;
            }
            let byte = if font_offset + row < FONT_8X16.len() {
                FONT_8X16[font_offset + row]
            } else {
                0
            };
            if byte == 0 {
                continue;
            }
            let slant = if row < 5 { 2usize } else if row < 11 { 1usize } else { 0usize };
            let row_ptr = fb.add(py * pitch) as *mut u32;
            for col in 0..8usize {
                if (byte & (0x80 >> col)) != 0 {
                    let px = cur_x + col + slant;
                    if px < scr_w {
                        *row_ptr.add(px) = color;
                    }
                    if bold && px + 1 < scr_w {
                        *row_ptr.add(px + 1) = color;
                    }
                }
            }
        }
        cur_x += 8;
    }
}

unsafe fn render_wordpad(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.wordpad.last_canvas = (cx, cy, cw, ch);

    // Slate gray background representing modern document workspace
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00475569);

    // 1. Formatting Ribbon Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);

    // Style buttons: [ Negrito B ] [ Itálico I ] [ Sublinhado U ]
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 8, cy + 5, 34, 26, "B", state.wordpad.bold);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 46, cy + 5, 34, 26, "I", state.wordpad.italic);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 84, cy + 5, 34, 26, "U", state.wordpad.underline);

    // Color Selector Chips (Preto, Azul, Vermelho, Verde)
    let colors = [0x000F172A, 0x001D4ED8, 0x00DC2626, 0x0016A34A];
    for (idx, &col) in colors.iter().enumerate() {
        let chip_x = cx + 130 + idx * 26;
        let is_sel = state.wordpad.color_idx == idx;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, chip_x, cy + 8, 20, 20, 4, col);
        if is_sel {
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, chip_x, cy + 8, 20, 20, 4, 0x0038BDF8);
        }
    }

    // Alignment buttons: [ < ] [ = ] [ > ]
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 246, cy + 5, 30, 26, "<", state.wordpad.align_mode == 0);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 280, cy + 5, 30, 26, "=", state.wordpad.align_mode == 1);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 314, cy + 5, 30, 26, ">", state.wordpad.align_mode == 2);

    // Document Action buttons
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 356, cy + 5, 80, 26, "Abrir", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 440, cy + 5, 80, 26, "Salvar", true);

    // 2. Graduated Ruler (18px)
    let ruler_y = cy + 36;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, ruler_y, cw, 18, 0, 0x00F1F5F9);
    for px in 0..cw {
        let rx = cx + px;
        if rx >= scr_w { break; }
        if px % 50 == 0 {
            // Major cm tick + label
            for py in (ruler_y + 8)..(ruler_y + 18) {
                if py < scr_h { *(fb.add(py * pitch) as *mut u32).add(rx) = 0x00475569; }
            }
            let cm_val = px / 50;
            let cm_s = format!("{}", cm_val);
            draw_text_px(fb, pitch, scr_w, scr_h, rx + 2, ruler_y + 2, &cm_s, 0x00475569, None);
        } else if px % 10 == 0 {
            // Minor tick
            for py in (ruler_y + 13)..(ruler_y + 18) {
                if py < scr_h { *(fb.add(py * pitch) as *mut u32).add(rx) = 0x0094A3B8; }
            }
        }
    }

    // 3. Centralized Simulated A4 White Paper
    let paper_w = (cw.saturating_sub(40)).min(520);
    let paper_h = ch.saturating_sub(36 + 18 + 24);
    let paper_x = cx + (cw.saturating_sub(paper_w)) / 2;
    let paper_y = ruler_y + 18 + 4;

    // Drop Shadow behind paper
    draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, paper_x, paper_y, paper_w, paper_h, 8, 4, 3, 40);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, paper_x, paper_y, paper_w, paper_h, 4, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, paper_x, paper_y, paper_w, paper_h, 4, 0x00CBD5E1);

    // Render formatted text inside paper sheet
    let margin_x = 24usize;
    let margin_y = 20usize;
    let text_area_w = paper_w.saturating_sub(margin_x * 2);
    let max_cols = text_area_w / 8;
    let text_col = state.wordpad.current_color();

    let mut row_idx = 0;
    for line in state.wordpad.text.lines() {
        if margin_y + row_idx * 20 >= paper_h - 10 { break; }
        let mut display = String::from(line);
        if display.len() > max_cols { display.truncate(max_cols); }

        let line_w = display.len() * 8;
        let line_x = match state.wordpad.align_mode {
            1 => paper_x + margin_x + (text_area_w.saturating_sub(line_w)) / 2, // Centro
            2 => paper_x + margin_x + text_area_w.saturating_sub(line_w),       // Direita
            _ => paper_x + margin_x,                                            // Esquerda
        };
        let line_y = paper_y + margin_y + row_idx * 20;

        // Styled text (Bold + Real Italic slant)
        draw_styled_text_px(
            fb, pitch, scr_w, scr_h,
            line_x, line_y, &display, text_col,
            state.wordpad.bold, state.wordpad.italic,
        );

        // Underline simulation
        if state.wordpad.underline && !display.trim().is_empty() {
            let u_y = line_y + 15;
            if u_y < scr_h {
                let row_ptr = fb.add(u_y * pitch) as *mut u32;
                for px in line_x..(line_x + line_w).min(scr_w) {
                    *row_ptr.add(px) = text_col;
                }
            }
        }

        row_idx += 1;
    }

    // 4. Status Bar
    let status_y = cy + ch - 22;
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, "Folha A4 | 210 x 297 mm | Zoom 100%", &state.wordpad.status_msg);
}

fn handle_wordpad_click(wp: &mut WordPadState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // Toolbar buttons (y: 0..36)
    if ly <= 36 {
        // [B] [I] [U]
        if lx >= 8 && lx <= 42 {
            wp.bold = !wp.bold;
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 46 && lx <= 80 {
            wp.italic = !wp.italic;
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 84 && lx <= 118 {
            wp.underline = !wp.underline;
            desk.needs_full_redraw = true;
            return true;
        }

        // Color chips (lx: 130..234)
        for idx in 0..4 {
            let cx = 130 + idx * 26;
            if lx >= cx && lx <= cx + 22 {
                wp.color_idx = idx;
                desk.needs_full_redraw = true;
                return true;
            }
        }

        // Alignments [ < ] [ = ] [ > ]
        if lx >= 246 && lx <= 276 {
            wp.align_mode = 0; // Left
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 280 && lx <= 310 {
            wp.align_mode = 1; // Center
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 314 && lx <= 344 {
            wp.align_mode = 2; // Right
            desk.needs_full_redraw = true;
            return true;
        }

        // [ Abrir ] [ Salvar ]
        if lx >= 356 && lx <= 436 {
            wp.load_from_vfs();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 440 && lx <= 520 {
            wp.save_to_vfs();
            desk.needs_full_redraw = true;
            return true;
        }
    }
    false
}

// ============================================================================
// 36. Calculadora Multi-Modo
// ============================================================================

unsafe fn render_calculator(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.calc.last_canvas = (cx, cy, cw, ch);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC);

    // 1. Mode Tabs Strip (28px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 28);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 8, cy + 2, 80, 24, "Padrao", state.calc.mode == 0);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 92, cy + 2, 95, 24, "Cientifica", state.calc.mode == 1);
    draw_glass_tab(fb, pitch, scr_w, scr_h, cx + 191, cy + 2, 110, 24, "Programador", state.calc.mode == 2);

    // 2. Vitrified Display Screen with Calculation History
    let disp_y = cy + 34;
    let disp_h = if state.calc.mode == 2 { 90 } else { 64 };
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, disp_y, cw - 24, disp_h, 6, 0x000F172A);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, disp_y, cw - 24, disp_h, 6, 0x0038BDF8);

    if state.calc.mode == 2 {
        // Programmer Mode simultaneous Radix View
        let mask = state.calc.programmer_mask();
        let val = state.calc.prog_val & mask;
        let hex_s = if let Some(op) = state.calc.prog_op {
            format!("HEX: 0x{:08X}  [0x{:X} {}]", val, state.calc.prog_acc & mask, op)
        } else {
            format!("HEX: 0x{:08X}", val)
        };
        let dec_s = format!("DEC: {}", val);
        let oct_s = format!("OCT: {:o}", val);
        let bin_s = format!("BIN: {:016b}", val);

        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 8, cx + cw - 20, &hex_s, 0x0038BDF8, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 26, cx + cw - 20, &dec_s, 0x00F8FAFC, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 44, cx + cw - 20, &oct_s, 0x00FBBF24, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 62, cx + cw - 20, &bin_s, 0x004ADE80, None);
    } else {
        // Standard & Scientific History & Display
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 10, cx + cw - 24, &state.calc.history, 0x0094A3B8, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 22, disp_y + 32, cx + cw - 24, &state.calc.display, 0x00FFFFFF, None);
    }

    // 3. Keypad Matrix
    let pad_y = disp_y + disp_h + 10;
    match state.calc.mode {
        0 => render_standard_keypad(fb, pitch, scr_w, scr_h, cx, pad_y, cw),
        1 => render_scientific_keypad(fb, pitch, scr_w, scr_h, cx, pad_y, cw),
        2 => render_programmer_keypad(fb, pitch, scr_w, scr_h, cx, pad_y, cw, state.calc.prog_word_size),
        _ => {}
    }
}

unsafe fn render_standard_keypad(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, cx: usize, y: usize, cw: usize) {
    let keys = [
        ["CE", "C", "<-", "/"],
        ["7", "8", "9", "*"],
        ["4", "5", "6", "-"],
        ["1", "2", "3", "+"],
        ["+/-", "0", ".", "="],
    ];
    let kw = cw.saturating_sub(36) / 4;
    let kh = 34usize;
    for (r, row) in keys.iter().enumerate() {
        for (c, &label) in row.iter().enumerate() {
            let kx = cx + 12 + c * (kw + 4);
            let ky = y + r * (kh + 6);
            let is_eq = label == "=";
            let is_op = label == "+" || label == "-" || label == "*" || label == "/";
            let bg = if is_eq { 0x000284C7 } else if is_op { 0x00E0F2FE } else { 0x00FFFFFF };
            fill_rounded_rect(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, bg);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, 0x00CBD5E1);
            let col = if is_eq { 0x00FFFFFF } else { 0x000F172A };
            let tx = kx + (kw.saturating_sub(label.len() * 8)) / 2;
            draw_text_px(fb, pitch, scr_w, scr_h, tx, ky + 9, label, col, None);
        }
    }
}

unsafe fn render_scientific_keypad(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, cx: usize, y: usize, cw: usize) {
    let keys = [
        ["sin", "cos", "tan", "log", "ln"],
        ["sqrt", "pow", "inv", "n!", "C"],
        ["7", "8", "9", "/", "<-"],
        ["4", "5", "6", "*", "+/-"],
        ["1", "2", "3", "-", "="],
        ["0", ".", "pi", "e", "+"],
    ];
    let kw = cw.saturating_sub(40) / 5;
    let kh = 30usize;
    for (r, row) in keys.iter().enumerate() {
        for (c, &label) in row.iter().enumerate() {
            let kx = cx + 12 + c * (kw + 4);
            let ky = y + r * (kh + 4);
            let is_eq = label == "=";
            let bg = if is_eq { 0x000284C7 } else { 0x00FFFFFF };
            fill_rounded_rect(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, bg);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, 0x00CBD5E1);
            let col = if is_eq { 0x00FFFFFF } else { 0x000F172A };
            let tx = kx + (kw.saturating_sub(label.len() * 8)) / 2;
            draw_text_px(fb, pitch, scr_w, scr_h, tx, ky + 7, label, col, None);
        }
    }
}

unsafe fn render_programmer_keypad(fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize, cx: usize, y: usize, cw: usize, w_size: usize) {
    // Word size toggles
    let sizes = ["QWORD", "DWORD", "WORD", "BYTE"];
    let sw = cw.saturating_sub(36) / 4;
    for (idx, &s_name) in sizes.iter().enumerate() {
        let sx = cx + 12 + idx * (sw + 4);
        draw_glass_button(fb, pitch, scr_w, scr_h, sx, y, sw, 24, s_name, w_size == idx);
    }

    let keys = [
        ["A", "B", "C", "AND", "OR"],
        ["D", "E", "F", "XOR", "NOT"],
        ["7", "8", "9", "<<", ">>"],
        ["4", "5", "6", "%", "CLR"],
        ["1", "2", "3", "0", "="],
    ];
    let kw = cw.saturating_sub(40) / 5;
    let kh = 30usize;
    let py_start = y + 30;
    for (r, row) in keys.iter().enumerate() {
        for (c, &label) in row.iter().enumerate() {
            let kx = cx + 12 + c * (kw + 4);
            let ky = py_start + r * (kh + 4);
            let is_eq = label == "=";
            let bg = if is_eq { 0x000284C7 } else { 0x00FFFFFF };
            fill_rounded_rect(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, bg);
            draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kx, ky, kw, kh, 4, 0x00CBD5E1);
            let col = if is_eq { 0x00FFFFFF } else { 0x000F172A };
            let tx = kx + (kw.saturating_sub(label.len() * 8)) / 2;
            draw_text_px(fb, pitch, scr_w, scr_h, tx, ky + 7, label, col, None);
        }
    }
}

fn handle_calc_click(calc: &mut CalculatorState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // Mode tabs (y: 0..28)
    if ly <= 28 {
        if lx >= 8 && lx <= 88 {
            calc.mode = 0;
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 92 && lx <= 187 {
            calc.mode = 1;
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 191 && lx <= 301 {
            calc.mode = 2;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    let cw = calc.last_canvas.2.max(320);

    match calc.mode {
        0 => {
            // Standard keypad hitboxes (matches render_calculator: disp_y=34, disp_h=64, +10 => pad_y=108)
            let pad_y = 108usize;
            let col_stride = (cw.saturating_sub(36) / 4) + 4;
            if ly >= pad_y && lx >= 12 && col_stride > 0 {
                let row = (ly - pad_y) / 40;
                let col = (lx - 12) / col_stride;
                if row < 5 && col < 4 {
                    let keys = [
                        ["CE", "C", "<-", "/"],
                        ["7", "8", "9", "*"],
                        ["4", "5", "6", "-"],
                        ["1", "2", "3", "+"],
                        ["+/-", "0", ".", "="],
                    ];
                    let k = keys[row][col];
                    match k {
                        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => calc.input_digit(k.chars().next().unwrap()),
                        "." => calc.input_dot(),
                        "+" | "-" | "*" | "/" => calc.set_op(k.chars().next().unwrap()),
                        "=" => calc.calculate_equals(),
                        "C" => calc.clear_all(),
                        "CE" => calc.clear_entry(),
                        "<-" => calc.backspace(),
                        "+/-" => calc.scientific_func("neg"),
                        _ => {}
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        1 => {
            // Scientific keypad (pad_y=108, 5 columns, stride ((cw-40)/5)+4)
            let pad_y = 108usize;
            let col_stride = (cw.saturating_sub(40) / 5) + 4;
            if ly >= pad_y && lx >= 12 && col_stride > 0 {
                let row = (ly - pad_y) / 34;
                let col = (lx - 12) / col_stride;
                if row < 6 && col < 5 {
                    let keys = [
                        ["sin", "cos", "tan", "log", "ln"],
                        ["sqrt", "pow", "inv", "n!", "C"],
                        ["7", "8", "9", "/", "<-"],
                        ["4", "5", "6", "*", "+/-"],
                        ["1", "2", "3", "-", "="],
                        ["0", ".", "pi", "e", "+"],
                    ];
                    let k = keys[row][col];
                    match k {
                        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => calc.input_digit(k.chars().next().unwrap()),
                        "." => calc.input_dot(),
                        "+" | "-" | "*" | "/" => calc.set_op(k.chars().next().unwrap()),
                        "pow" => calc.set_op('^'),
                        "=" => calc.calculate_equals(),
                        "C" => calc.clear_all(),
                        "<-" => calc.backspace(),
                        "sin" | "cos" | "tan" | "log" | "ln" | "sqrt" | "inv" | "n!" => calc.scientific_func(k),
                        "+/-" => calc.scientific_func("neg"),
                        "pi" => {
                            calc.display = format!("{:.6}", core::f64::consts::PI);
                            calc.clear_on_next = true;
                        }
                        "e" => {
                            calc.display = format!("{:.6}", core::f64::consts::E);
                            calc.clear_on_next = true;
                        }
                        _ => {}
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        2 => {
            // Programmer mode: disp_y=34, disp_h=90, +10 => toggles at ly: 134..=158
            let sw_stride = (cw.saturating_sub(36) / 4) + 4;
            if ly >= 134 && ly <= 158 && lx >= 12 && sw_stride > 0 {
                let col = (lx - 12) / sw_stride;
                if col < 4 {
                    calc.prog_word_size = col;
                    calc.prog_apply_mask();
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
            // Keypad starts at y + 30 = 164
            let pad_y = 164usize;
            let col_stride = (cw.saturating_sub(40) / 5) + 4;
            if ly >= pad_y && lx >= 12 && col_stride > 0 {
                let row = (ly - pad_y) / 34;
                let col = (lx - 12) / col_stride;
                if row < 5 && col < 5 {
                    let keys = [
                        ["A", "B", "C", "AND", "OR"],
                        ["D", "E", "F", "XOR", "NOT"],
                        ["7", "8", "9", "<<", ">>"],
                        ["4", "5", "6", "%", "CLR"],
                        ["1", "2", "3", "0", "="],
                    ];
                    let k = keys[row][col];
                    match k {
                        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                            calc.prog_input_hex((k.as_bytes()[0] - b'0') as u64);
                        }
                        "A" | "B" | "C" | "D" | "E" | "F" => {
                            calc.prog_input_hex((k.as_bytes()[0] - b'A' + 10) as u64);
                        }
                        "AND" => calc.prog_set_op('&'),
                        "OR" => calc.prog_set_op('|'),
                        "XOR" => calc.prog_set_op('^'),
                        "%" => calc.prog_set_op('%'),
                        "=" => calc.prog_equals(),
                        "CLR" => calc.prog_clear(),
                        "NOT" => {
                            calc.prog_val = !calc.prog_val;
                            calc.prog_clear_on_next = false;
                            calc.prog_apply_mask();
                        }
                        "<<" => {
                            calc.prog_val = calc.prog_val.wrapping_shl(1);
                            calc.prog_clear_on_next = false;
                            calc.prog_apply_mask();
                        }
                        ">>" => {
                            calc.prog_val >>= 1;
                            calc.prog_clear_on_next = false;
                            calc.prog_apply_mask();
                        }
                        _ => {}
                    }
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
        _ => {}
    }

    false
}

// ============================================================================
// 37. Notas Autoadesivas (Sticky Notes)
// ============================================================================

unsafe fn render_sticky_notes(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.stickies.last_canvas = (cx, cy, cw, ch);

    // Desktop background for sticky notes board (Soft Cork / Slate)
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00E2E8F0);

    // Top control strip
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 32);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 8, cy + 4, 110, 24, "+ Nova Nota", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 126, cy + 4, 110, 24, "Mudar Cor", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 244, cy + 4, 110, 24, "Excluir Nota", false);

    // Render each sticky note
    let palettes = [
        // (Body, Header, Text, Border)
        (0x00FEF08A, 0x00FDE047, 0x00713F12, 0x00EAB308), // Amarelo Canário
        (0x00BAE6FD, 0x007DD3FC, 0x000C4A6E, 0x000284C7), // Azul Pastel
        (0x00FBCFE8, 0x00F472B6, 0x00831843, 0x00DB2777), // Rosa Suave
        (0x00BBF7D0, 0x0086EFAC, 0x0014532D, 0x0016A34A), // Verde Limão
    ];

    for (idx, note) in state.stickies.notes.iter().enumerate() {
        let (body_c, head_c, text_c, border_c) = palettes[note.color_idx % 4];
        let is_active = idx == state.stickies.active_idx;

        let nx = cx + note.x;
        let ny = cy + note.y;
        if nx + note.w > cx + cw || ny + note.h > cy + ch { continue; }

        // Drop shadow
        draw_soft_drop_shadow(fb, pitch, scr_w, scr_h, nx, ny, note.w, note.h, 6, 3, 2, 35);

        // Body
        fill_rounded_rect(fb, pitch, scr_w, scr_h, nx, ny, note.w, note.h, 6, body_c);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, nx, ny, note.w, note.h, 6, if is_active { 0x0038BDF8 } else { border_c });

        // Header bar with 3D crystal shine
        fill_rounded_rect(fb, pitch, scr_w, scr_h, nx, ny, note.w, 24, 6, head_c);
        let note_title = format!("Nota #{}", note.id);
        draw_text_px(fb, pitch, scr_w, scr_h, nx + 8, ny + 4, &note_title, text_c, None);

        // Header control icons: [+] and [X]
        draw_text_px(fb, pitch, scr_w, scr_h, nx + note.w - 38, ny + 4, "+", text_c, None);
        draw_text_px(fb, pitch, scr_w, scr_h, nx + note.w - 18, ny + 4, "x", text_c, None);

        // Note Content Lines
        let max_cols = (note.w - 16) / 8;
        let mut r_idx = 0;
        for line in note.text.lines() {
            if 28 + r_idx * 18 >= note.h - 10 { break; }
            let mut chunk = String::from(line);
            if chunk.len() > max_cols { chunk.truncate(max_cols); }
            draw_text_px(fb, pitch, scr_w, scr_h, nx + 10, ny + 28 + r_idx * 18, &chunk, text_c, None);
            r_idx += 1;
        }
    }

    // Status Bar
    let status_y = cy + ch - 22;
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, "Notas Autoadesivas VFS", &state.stickies.status_msg);
}

fn handle_stickies_click(st: &mut StickyNotesState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // Toolbar buttons (y: 0..32)
    if ly <= 32 {
        if lx >= 8 && lx <= 118 {
            st.add_note();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 126 && lx <= 236 {
            st.cycle_color();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 244 && lx <= 354 {
            st.delete_active();
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Check click on notes (reverse Z-order)
    for idx in (0..st.notes.len()).rev() {
        let (nx, ny, nw, nh) = {
            let n = &st.notes[idx];
            (n.x, n.y, n.w, n.h)
        };
        if lx >= nx && lx <= nx + nw && ly >= ny && ly <= ny + nh {
            st.active_idx = idx;

            // Click on '+' in note header
            if ly >= ny && ly <= ny + 24 && lx >= nx + nw - 42 && lx <= nx + nw - 24 {
                st.add_note();
                desk.needs_full_redraw = true;
                return true;
            }
            // Click on 'X' in note header
            if ly >= ny && ly <= ny + 24 && lx >= nx + nw - 24 && lx <= nx + nw {
                st.delete_active();
                desk.needs_full_redraw = true;
                return true;
            }

            desk.needs_full_redraw = true;
            return true;
        }
    }

    false
}

// ============================================================================
// 38. Calendário, Relógio Mundial e Cronômetro
// ============================================================================

unsafe fn render_calendar_clock(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.calendar.last_canvas = (cx, cy, cw, ch);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC);

    // Read real hardware CMOS RTC
    let rtc = read_rtc();

    // 1. Left Column: Interactive Monthly Calendar (width: 220px, lx: 10..230)
    let cal_w = 220usize;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 10, cy + 10, cal_w, ch.saturating_sub(20), 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 10, cy + 10, cal_w, ch.saturating_sub(20), 6, 0x00CBD5E1);

    // Month Navigation Header
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 14, cy + 16, 24, 24, "<", false);
    let month_names = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];
    let m_idx = (state.calendar.month.clamp(1, 12) - 1) as usize;
    let header_text = format!("{} {}", month_names[m_idx], state.calendar.year);
    draw_text_px(fb, pitch, scr_w, scr_h, cx + 82, cy + 20, &header_text, 0x000F172A, None);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 200, cy + 16, 24, 24, ">", false);

    // Days of week (7 columns x 29px stride inside 220px)
    let dows = ["D", "S", "T", "Q", "Q", "S", "S"];
    for (i, &d) in dows.iter().enumerate() {
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 22 + i * 29, cy + 50, d, 0x0064748B, None);
    }
    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 16, cy + 68, cal_w - 12, 1, 0, 0x00E2E8F0);

    // Calendar Days Grid
    let first_dow = calc_day_of_week(state.calendar.year, state.calendar.month, 1) as usize;
    let total_days = days_in_month(state.calendar.year, state.calendar.month);

    let mut day_num = 1;
    for row in 0..6usize {
        for col in 0..7usize {
            if row == 0 && col < first_dow { continue; }
            if day_num > total_days { break; }

            let dx = cx + 18 + col * 29;
            let dy = cy + 76 + row * 28;

            let is_today = state.calendar.year == rtc.year as i32 && state.calendar.month == rtc.month as u32 && day_num == rtc.day as u32;
            let is_sel = day_num == state.calendar.selected_day;

            if is_sel {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, dx - 2, dy - 2, 26, 24, 4, 0x00BAE6FD);
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, dx - 2, dy - 2, 26, 24, 4, 0x000284C7);
            } else if is_today {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, dx - 2, dy - 2, 26, 24, 4, 0x00FEF08A);
            }

            let ds = format!("{:>2}", day_num);
            let col_t = if is_sel { 0x000C4A6E } else if is_today { 0x00713F12 } else { 0x000F172A };
            draw_text_px(fb, pitch, scr_w, scr_h, dx, dy + 2, &ds, col_t, None);

            day_num += 1;
        }
    }

    // 2. Center Column: Vitrified Analog Clock & World Clocks (lx: 236..432, width: 196px)
    let clk_x = cx + 236;
    let clk_w = 196usize;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, clk_x, cy + 10, clk_w, 196, 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, clk_x, cy + 10, clk_w, 196, 6, 0x00CBD5E1);

    // Circular Glass Clock Face (Center at clk_x + 98, cy + 80, radius: 52)
    let center_x = clk_x + clk_w / 2;
    let center_y = cy + 80;
    let radius = 52isize;

    // Draw clock face background & border
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius {
                let px = (center_x as isize + dx) as usize;
                let py = (center_y as isize + dy) as usize;
                if px < scr_w && py < scr_h {
                    let ptr = fb.add(py * pitch) as *mut u32;
                    let is_rim = dist_sq >= (radius - 2) * (radius - 2);
                    *ptr.add(px) = if is_rim { 0x000284C7 } else { 0x00F0F9FF };
                }
            }
        }
    }

    // Draw 12 hour ticks
    for h in 1..=12 {
        let rad = (h as f32) * (core::f32::consts::PI / 6.0);
        let tx = center_x as f32 + (radius as f32 - 8.0) * rad.sin();
        let ty = center_y as f32 - (radius as f32 - 8.0) * rad.cos();
        fill_rounded_rect(fb, pitch, scr_w, scr_h, tx as usize - 1, ty as usize - 1, 3, 3, 1, 0x000F172A);
    }

    // Calculate hand angles
    let sec_rad = (rtc.sec as f32) * (core::f32::consts::PI / 30.0);
    let min_rad = ((rtc.min as f32) + (rtc.sec as f32) / 60.0) * (core::f32::consts::PI / 30.0);
    let hour_rad = ((rtc.hour % 12) as f32 + (rtc.min as f32) / 60.0) * (core::f32::consts::PI / 6.0);

    // Hour Hand (length: 26)
    let hx = center_x as isize + (26.0 * hour_rad.sin()) as isize;
    let hy = center_y as isize - (26.0 * hour_rad.cos()) as isize;
    draw_line(fb, pitch, scr_w, scr_h, center_x as isize, center_y as isize, hx, hy, 0x000F172A);

    // Minute Hand (length: 39)
    let mx_hand = center_x as isize + (39.0 * min_rad.sin()) as isize;
    let my_hand = center_y as isize - (39.0 * min_rad.cos()) as isize;
    draw_line(fb, pitch, scr_w, scr_h, center_x as isize, center_y as isize, mx_hand, my_hand, 0x000284C7);

    // Second Hand (length: 45, Ruby-Red)
    let sx_hand = center_x as isize + (45.0 * sec_rad.sin()) as isize;
    let sy_hand = center_y as isize - (45.0 * sec_rad.cos()) as isize;
    draw_line(fb, pitch, scr_w, scr_h, center_x as isize, center_y as isize, sx_hand, sy_hand, 0x00EF4444);

    // Digital CMOS Time below clock
    let time_str = format!("{:02}:{:02}:{:02} (RTC)", rtc.hour, rtc.min, rtc.sec);
    draw_text_px(fb, pitch, scr_w, scr_h, clk_x + 42, cy + 150, &time_str, 0x000F172A, None);

    // World Clocks table below
    let wc_y = cy + 214;
    let wc_h = ch.saturating_sub(224);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, clk_x, wc_y, clk_w, wc_h, 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, clk_x, wc_y, clk_w, wc_h, 6, 0x00CBD5E1);

    draw_text_px(fb, pitch, scr_w, scr_h, clk_x + 10, wc_y + 8, "Fusos Horarios:", 0x001E3A8A, None);
    let zones = [
        ("Brasilia UTC-3", (rtc.hour as i32 - 3 + 24) % 24),
        ("Londres  UTC+0", rtc.hour as i32),
        ("New York UTC-5", (rtc.hour as i32 - 5 + 24) % 24),
        ("Toquio   UTC+9", (rtc.hour as i32 + 9) % 24),
    ];
    for (idx, (city, zh)) in zones.iter().enumerate() {
        let zy = wc_y + 28 + idx * 20;
        let z_str = format!("{:<15} {:02}:{:02}", city, zh, rtc.min);
        draw_text_clipped(fb, pitch, scr_w, scr_h, clk_x + 10, zy, clk_x + clk_w - 6, &z_str, 0x00475569, None);
    }

    // 3. Right Column: 100Hz Precision Stopwatch (lx: 438..cw-10, sw_w = cw.saturating_sub(448))
    let sw_x = cx + 438;
    let sw_w = cw.saturating_sub(448);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, sw_x, cy + 10, sw_w, ch.saturating_sub(20), 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sw_x, cy + 10, sw_w, ch.saturating_sub(20), 6, 0x00CBD5E1);

    draw_text_clipped(fb, pitch, scr_w, scr_h, sw_x + 10, cy + 16, sw_x + sw_w.saturating_sub(6), "Cronometro (100Hz)", 0x001E3A8A, None);

    // Large centesimal display (MM:SS.CC)
    let ticks = state.calendar.current_sw_ticks();
    let cs = ticks % 100;
    let sec = (ticks / 100) % 60;
    let min = (ticks / 6000) % 60;
    let sw_str = format!("{:02}:{:02}.{:02}", min, sec, cs);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, sw_x + 10, cy + 42, sw_w.saturating_sub(20), 46, 6, 0x000F172A);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, sw_x + 10, cy + 42, sw_w.saturating_sub(20), 46, 6, 0x0038BDF8);
    draw_text_px(fb, pitch, scr_w, scr_h, sw_x + 24, cy + 56, &sw_str, 0x004ADE80, None);

    // Control buttons: [ Iniciar / Pausar ] (60px) [ Zerar ] (56px) [ Volta ] (56px) with 6px spacing
    let btn_label = if state.calendar.sw_running { "Pausar" } else { "Iniciar" };
    draw_glass_button(fb, pitch, scr_w, scr_h, sw_x + 10, cy + 98, 60, 26, btn_label, state.calendar.sw_running);
    draw_glass_button(fb, pitch, scr_w, scr_h, sw_x + 76, cy + 98, 56, 26, "Zerar", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, sw_x + 138, cy + 98, 56, 26, "Volta", false);

    // Laps table
    let lap_y = cy + 134;
    draw_text_px(fb, pitch, scr_w, scr_h, sw_x + 10, lap_y, "Voltas Registradas:", 0x0064748B, None);
    for (idx, &lap_t) in state.calendar.laps.iter().enumerate() {
        let ly = lap_y + 20 + idx * 18;
        if ly + 18 >= cy + ch - 15 { break; }
        let l_cs = lap_t % 100;
        let l_sec = (lap_t / 100) % 60;
        let l_min = (lap_t / 6000) % 60;
        let lap_s = format!("Volta {:>2}: {:02}:{:02}.{:02}", idx + 1, l_min, l_sec, l_cs);
        draw_text_clipped(fb, pitch, scr_w, scr_h, sw_x + 10, ly, sw_x + sw_w.saturating_sub(6), &lap_s, 0x000F172A, None);
    }
}

fn handle_calendar_clock_click(cal: &mut CalendarClockState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // Calendar month buttons: '<' (lx: 14..38) and '>' (lx: 200..224) at ly: 16..40
    if ly >= 16 && ly <= 40 {
        if lx >= 14 && lx <= 38 {
            cal.prev_month();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 200 && lx <= 224 {
            cal.next_month();
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Calendar day grid click (7 cols x 29px stride starting at lx=18, ly=76)
    if lx >= 16 && lx < 18 + 7 * 29 && ly >= 74 && ly < 76 + 6 * 28 {
        let row = ly.saturating_sub(76) / 28;
        let col = lx.saturating_sub(18) / 29;
        if row < 6 && col < 7 {
            let first_dow = calc_day_of_week(cal.year, cal.month, 1) as usize;
            let total = days_in_month(cal.year, cal.month);
            let cell_idx = row * 7 + col;
            if cell_idx >= first_dow {
                let day = (cell_idx - first_dow + 1) as u32;
                if day <= total {
                    cal.selected_day = day;
                    desk.needs_full_redraw = true;
                    return true;
                }
            }
        }
    }

    // Stopwatch buttons (exact same geometry as render_calendar_clock: sw_x = 438)
    let sw_x = 438usize;
    if ly >= 98 && ly <= 124 {
        if lx >= sw_x + 10 && lx <= sw_x + 70 {
            // Iniciar / Pausar
            cal.toggle_stopwatch();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= sw_x + 76 && lx <= sw_x + 132 {
            // Zerar
            cal.reset_stopwatch();
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= sw_x + 138 && lx <= sw_x + 194 {
            // Volta
            cal.record_lap();
            desk.needs_full_redraw = true;
            return true;
        }
    }

    false
}

// ============================================================================
// 39. Mapa de Caracteres e Visualizador de Fontes
// ============================================================================

unsafe fn render_charmap(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.charmap.last_canvas = (cx, cy, cw, ch);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC);

    // 1. Grid of Characters (ASCII 32 to 255) in 16 columns x 14 rows
    let grid_x = cx + 12;
    let grid_y = cy + 12;
    let cell_w = 22usize;
    let cell_h = 22usize;

    for row in 0..14 {
        for col in 0..16 {
            let ascii_val = 32 + row * 16 + col;
            if ascii_val > 255 { break; }

            let kx = grid_x + col * cell_w;
            let ky = grid_y + row * cell_h;
            let is_sel = state.charmap.selected_char as usize == ascii_val;

            if is_sel {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, kx, ky, cell_w, cell_h, 3, 0x00BAE6FD);
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kx, ky, cell_w, cell_h, 3, 0x000284C7);
            } else {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, kx, ky, cell_w, cell_h, 2, 0x00FFFFFF);
                draw_rounded_rect_border(fb, pitch, scr_w, scr_h, kx, ky, cell_w, cell_h, 2, 0x00E2E8F0);
            }

            // Draw glyph
            let ch = ascii_val as u8 as char;
            let ch_s = format!("{}", ch);
            draw_text_px(fb, pitch, scr_w, scr_h, kx + 7, ky + 3, &ch_s, 0x000F172A, None);
        }
    }

    // 2. Right Side: 4x Magnified Preview & Info Box
    let right_x = grid_x + 16 * cell_w + 18;
    let right_w = cw.saturating_sub(right_x - cx + 10);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, right_x, cy + 12, right_w, ch - 40, 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, right_x, cy + 12, right_w, ch - 40, 6, 0x00CBD5E1);

    draw_text_px(fb, pitch, scr_w, scr_h, right_x + 12, cy + 20, "Pre-visualizacao (4x):", 0x001E3A8A, None);

    // 4x Magnified Glyph Box (32x64 pixels from 8x16 font)
    let box_x = right_x + (right_w.saturating_sub(40)) / 2;
    let box_y = cy + 44;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, box_x - 4, box_y - 4, 40, 72, 4, 0x000F172A);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, box_x - 4, box_y - 4, 40, 72, 4, 0x0038BDF8);

    let sel_ascii = state.charmap.selected_char as usize;
    let font_offset = if sel_ascii >= 32 && sel_ascii < 128 {
        (sel_ascii - 32) * 16
    } else {
        (b'?' as usize - 32) * 16
    };

    for row in 0..16 {
        let byte = if font_offset + row < FONT_8X16.len() { FONT_8X16[font_offset + row] } else { 0 };
        for col in 0..8 {
            if (byte & (0x80 >> col)) != 0 {
                // 4x scaling: draw 4x4 block
                let px_base = box_x + col * 4;
                let py_base = box_y + row * 4;
                for by_p in 0..4 {
                    for bx_p in 0..4 {
                        let px = px_base + bx_p;
                        let py = py_base + by_p;
                        if px < scr_w && py < scr_h {
                            *(fb.add(py * pitch) as *mut u32).add(px) = 0x0038BDF8;
                        }
                    }
                }
            }
        }
    }

    // ASCII Metadata
    let info_y = box_y + 80;
    let dec_s = format!("Dec: {:>3}", sel_ascii);
    let hex_s = format!("Hex: 0x{:02X}", sel_ascii);
    let oct_s = format!("Oct: 0o{:03o}", sel_ascii);
    let char_s = format!("Char: '{}'", sel_ascii as u8 as char);

    draw_text_px(fb, pitch, scr_w, scr_h, right_x + 16, info_y, &dec_s, 0x000F172A, None);
    draw_text_px(fb, pitch, scr_w, scr_h, right_x + 16, info_y + 18, &hex_s, 0x000284C7, None);
    draw_text_px(fb, pitch, scr_w, scr_h, right_x + 16, info_y + 36, &oct_s, 0x0064748B, None);
    draw_text_px(fb, pitch, scr_w, scr_h, right_x + 16, info_y + 54, &char_s, 0x0016A34A, None);

    // Button: "Copiar Caractere"
    let btn_y = info_y + 84;
    draw_glass_button(fb, pitch, scr_w, scr_h, right_x + 16, btn_y, right_w - 32, 28, "Copiar Caractere", true);

    // 3. Status Bar
    let status_y = cy + ch - 22;
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, "Tabela ASCII / ANSI CP1252", &state.charmap.status_msg);
}

fn handle_charmap_click(cm: &mut CharMapState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    let grid_x = 12usize;
    let grid_y = 12usize;
    let cell_w = 22usize;
    let cell_h = 22usize;

    // Check click in 16x14 grid
    if lx >= grid_x && lx < grid_x + 16 * cell_w && ly >= grid_y && ly < grid_y + 14 * cell_h {
        let col = (lx - grid_x) / cell_w;
        let row = (ly - grid_y) / cell_h;
        let ascii = 32 + row * 16 + col;
        if ascii <= 255 {
            cm.selected_char = ascii as u8;
            cm.status_msg = format!("Caractere selecionado: '{}' (ASCII {})", ascii as u8 as char, ascii);
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Button "Copiar Caractere" (rendered at btn_y = cy + 208, h = 28 -> ly: 208..=236)
    let right_x = grid_x + 16 * cell_w + 18;
    let cw = cm.last_canvas.2;
    let right_w = cw.saturating_sub(right_x + 10);
    let btn_right = if right_w > 32 { right_x + right_w - 16 } else { usize::MAX };
    if lx >= right_x + 16 && lx <= btn_right && ly >= 208 && ly <= 236 {
        let ch_s = format!("{}", cm.selected_char as char);
        GLOBAL_CLIPBOARD.lock().push(ch_s.clone());
        cm.status_msg = format!("Caractere '{}' copiado para a area de transferencia!", ch_s);
        desk.needs_full_redraw = true;
        return true;
    }

    false
}

// ============================================================================
// 40. Gerenciador de Histórico do Clipboard
// ============================================================================

unsafe fn render_clipboard_mgr(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    {
        let mut state = CAT5_STATE.lock();
        state.clipboard_canvas = (cx, cy, cw, ch);
    }

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC);

    // 1. Toolbar Strip (34px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 34);
    draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 12, cy + 9, cx + cw.saturating_sub(168), "Historico da Area de Transferencia (10 Slots)", 0x001E3A8A, None);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + cw.saturating_sub(160), cy + 4, 150, 26, "Limpar Historico", false);

    let clip = GLOBAL_CLIPBOARD.lock();

    // 2. List of 10 items
    let list_y = cy + 40;
    let item_h = 30usize;

    for idx in 0..10 {
        let iy = list_y + idx * (item_h + 4);
        if iy + item_h >= cy + ch - 22 { break; }

        let is_active = idx == clip.active_index;
        let bg = if is_active { 0x00E0F2FE } else { 0x00FFFFFF };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 12, iy, cw - 24, item_h, 4, bg);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 12, iy, cw - 24, item_h, 4, if is_active { 0x000284C7 } else { 0x00E2E8F0 });

        // Badge [ # ]
        let badge = format!("[ {:>2} ]", idx + 1);
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 20, iy + 7, &badge, 0x000284C7, None);

        // Content
        if let Some(text) = clip.items.get(idx) {
            let mut preview = text.replace('\n', " ");
            let max_c = (cw.saturating_sub(180)) / 8;
            if preview.len() > max_c { preview.truncate(max_c); }
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 70, iy + 7, &preview, 0x000F172A, None);

            // Size
            let sz_str = format!("({} B)", text.len());
            draw_text_px(fb, pitch, scr_w, scr_h, cx + cw - 90, iy + 7, &sz_str, 0x0094A3B8, None);
        } else {
            draw_text_px(fb, pitch, scr_w, scr_h, cx + 70, iy + 7, "<Vazio>", 0x0094A3B8, None);
        }
    }

    // 3. Status Bar
    let status_y = cy + ch - 22;
    let stat = format!("Total: {} itens | Ativo: Slot #{}", clip.items.len(), clip.active_index + 1);
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, &stat, "Clique em um item para copiar");
}

fn handle_clipboard_click(lx: usize, ly: usize, cw: usize, desk: &mut DesktopState) -> bool {
    // Limpar Histórico button (rendered at lx: cw-160..cw-10, ly: 4..30)
    if cw >= 160 && lx >= cw.saturating_sub(160) && lx <= cw.saturating_sub(10) && ly >= 4 && ly <= 30 {
        GLOBAL_CLIPBOARD.lock().clear();
        desk.needs_full_redraw = true;
        return true;
    }

    // Click item in list to select
    let list_y = 40usize;
    if ly >= list_y {
        let idx = (ly - list_y) / 34;
        let mut clip = GLOBAL_CLIPBOARD.lock();
        if idx < clip.items.len() {
            clip.active_index = idx;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    false
}

// ============================================================================
// 41. Catálogo de Contatos (Address Book)
// ============================================================================

unsafe fn render_address_book(
    fb: *mut u8, pitch: usize, scr_w: usize, scr_h: usize,
    cx: usize, cy: usize, cw: usize, ch: usize,
    _desk: &mut DesktopState,
) {
    let mut state = CAT5_STATE.lock();
    state.contacts.last_canvas = (cx, cy, cw, ch);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, cy, cw, ch, 0, 0x00F8FAFC);

    // 1. Top Ribbon Toolbar (36px)
    draw_glass_toolbar(fb, pitch, scr_w, scr_h, cx, cy, cw, 36);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 8, cy + 5, 140, 26, "+ Novo Contato", true);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 154, cy + 5, 80, 26, "Editar", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 240, cy + 5, 80, 26, "Remover", false);
    draw_glass_button(fb, pitch, scr_w, scr_h, cx + 326, cy + 5, 150, 26, "Exportar para CSV", true);

    // 2. Left Column: Search & Contact List (width: 220px)
    let list_w = 220usize;
    let body_y = cy + 36;
    let body_h = ch.saturating_sub(36 + 22);

    fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, body_y, list_w, body_h, 0, 0x00F1F5F9);

    // Quick Search Input (lx: 8..212, ly: 42..66)
    let search_focused = state.contacts.edit_field == 5;
    let search_display = if state.contacts.search_query.is_empty() && !search_focused {
        "Buscar contatos..."
    } else {
        state.contacts.search_query.as_str()
    };
    draw_glass_textbox(fb, pitch, scr_w, scr_h, cx + 8, body_y + 6, list_w - 16, 24, search_display, search_focused);

    // Contacts items (filtered by search_query)
    let filtered = state.contacts.filtered_indices();
    let mut item_y = body_y + 36;
    for &idx in &filtered {
        if item_y + 40 >= body_y + body_h { break; }
        let c = &state.contacts.contacts[idx];
        let is_sel = idx == state.contacts.selected_idx;
        let bg = if is_sel { 0x00BAE6FD } else { 0x00FFFFFF };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx + 8, item_y, list_w - 16, 38, 4, bg);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, cx + 8, item_y, list_w - 16, 38, 4, if is_sel { 0x000284C7 } else { 0x00CBD5E1 });

        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 14, item_y + 4, cx + list_w - 12, &c.name, 0x000F172A, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 14, item_y + 20, cx + list_w - 12, &c.company, 0x0064748B, None);

        item_y += 42;
    }
    if filtered.is_empty() {
        draw_text_px(fb, pitch, scr_w, scr_h, cx + 14, body_y + 44, "Nenhum resultado", 0x0064748B, None);
    }

    // 3. Right Column: Contact Details Form
    let form_x = cx + list_w + 16;
    let form_w = cw.saturating_sub(list_w + 32);
    fill_rounded_rect(fb, pitch, scr_w, scr_h, form_x, body_y + 10, form_w, body_h - 20, 6, 0x00FFFFFF);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, form_x, body_y + 10, form_w, body_h - 20, 6, 0x00CBD5E1);

    draw_text_px(fb, pitch, scr_w, scr_h, form_x + 16, body_y + 20, "Ficha de Cadastro do Contato", 0x001E3A8A, None);

    if let Some(c) = state.contacts.contacts.get(state.contacts.selected_idx) {
        let fields = [
            ("Nome Completo:", &c.name, 0),
            ("E-mail:", &c.email, 1),
            ("Telefone:", &c.phone, 2),
            ("Endereco:", &c.address, 3),
            ("Empresa / Organizacao:", &c.company, 4),
        ];

        for (idx, (label, val, f_idx)) in fields.iter().enumerate() {
            let fy = body_y + 50 + idx * 46;
            draw_text_px(fb, pitch, scr_w, scr_h, form_x + 16, fy, label, 0x00475569, None);
            let is_focus = state.contacts.edit_field == *f_idx;
            draw_glass_textbox(fb, pitch, scr_w, scr_h, form_x + 16, fy + 16, form_w - 32, 24, val, is_focus);
        }
    }

    // 4. Status Bar
    let status_y = cy + ch - 22;
    draw_glass_statusbar(fb, pitch, scr_w, scr_h, cx, status_y, cw, 22, "Catalogo de Contatos HybridOS", &state.contacts.status_msg);
}

fn handle_contacts_click(ab: &mut AddressBookState, lx: usize, ly: usize, desk: &mut DesktopState) -> bool {
    // Toolbar buttons (y: 0..36)
    if ly <= 36 {
        if lx >= 8 && lx <= 148 {
            // Adicionar Contato
            ab.contacts.push(ContactItem {
                name: format!("Novo Contato #{}", ab.contacts.len() + 1),
                email: String::from("usuario@hybridos.org"),
                phone: String::from("+55 11 90000-0000"),
                address: String::from("Endereco Padrao"),
                company: String::from("Empresa"),
            });
            ab.selected_idx = ab.contacts.len() - 1;
            ab.status_msg = String::from("Contato adicionado");
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 154 && lx <= 234 {
            // Editar: avança campo de foco (0..=4)
            ab.edit_field = if ab.edit_field >= 5 { 0 } else { (ab.edit_field + 1) % 5 };
            desk.needs_full_redraw = true;
            return true;
        } else if lx >= 240 && lx <= 320 {
            // Remover
            if !ab.contacts.is_empty() {
                ab.contacts.remove(ab.selected_idx);
                if ab.selected_idx >= ab.contacts.len() && !ab.contacts.is_empty() {
                    ab.selected_idx = ab.contacts.len() - 1;
                }
                ab.status_msg = String::from("Contato removido");
                desk.needs_full_redraw = true;
                return true;
            }
        } else if lx >= 326 && lx <= 476 {
            // Exportar para CSV
            ab.export_csv();
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Focus quick search box (lx: 8..=212, ly: 42..=66)
    if lx >= 8 && lx <= 212 && ly >= 42 && ly <= 66 {
        ab.edit_field = 5;
        desk.needs_full_redraw = true;
        return true;
    }

    // Select contact from filtered left list
    let list_start_y = 36 + 36;
    if lx >= 8 && lx <= 212 && ly >= list_start_y {
        let row_idx = (ly - list_start_y) / 42;
        let filtered = ab.filtered_indices();
        if let Some(&real_idx) = filtered.get(row_idx) {
            ab.selected_idx = real_idx;
            if ab.edit_field == 5 {
                ab.edit_field = 0;
            }
            desk.needs_full_redraw = true;
            return true;
        }
    }

    // Select form field on right side
    if lx >= 236 && ly >= 86 {
        let f_idx = (ly - 86) / 46;
        if f_idx < 5 {
            ab.edit_field = f_idx;
            desk.needs_full_redraw = true;
            return true;
        }
    }

    false
}
