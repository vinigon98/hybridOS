//! ux_design_system.rs — Design System Unificado do HybridOS
//!
//! Centraliza paleta de cores, espaçamentos, raios de borda e componentes
//! reutilizáveis (tooltip, modal, toggle, slider) para todas as camadas de UI.
//! Implementado em #![no_std], sem dependência de GPU acelerada.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

// ============================================================================
// 1. Paleta de Cores (inspirada em Windows 11 / macOS / Material Design)
// ============================================================================

/// Fundo principal da área de trabalho (Slate 950)
pub const BG_DESKTOP: u32 = 0x000F172A;
/// Fundo de superfície (painéis, janelas inativas) (Slate 900)
pub const BG_SURFACE: u32 = 0x001E293B;
/// Fundo de janela ativa (Slate 800)
pub const BG_WINDOW_ACTIVE: u32 = 0x00334155;
/// Texto primário (Slate 50)
pub const TEXT_PRIMARY: u32 = 0x00F8FAFC;
/// Texto secundário (Slate 400)
pub const TEXT_SECONDARY: u32 = 0x00CBD5E1;
/// Texto mutado / legendas (Slate 500)
pub const TEXT_MUTED: u32 = 0x0094A3B8;
/// Destaque primário (Sapphire Blue 600)
pub const ACCENT_PRIMARY: u32 = 0x000284C7;
/// Destaque hover (Sky 400)
pub const ACCENT_HOVER: u32 = 0x0038BDF8;
/// Destaque pressionado (Blue 700)
pub const ACCENT_PRESSED: u32 = 0x000369A1;
/// Borda sutil (Slate 700)
pub const BORDER_SUBTLE: u32 = 0x00475569;
/// Borda focada (Sapphire)
pub const BORDER_FOCUSED: u32 = 0x0038BDF8;
/// Erro / Alertas críticos (Red 500)
pub const ERROR_COLOR: u32 = 0x00EF4444;
/// Sucesso (Emerald 500)
pub const SUCCESS_COLOR: u32 = 0x0010B981;
/// Aviso (Amber 500)
pub const WARNING_COLOR: u32 = 0x00F59E0B;
/// Sobreposição escura (menus, modais)
pub const OVERLAY_DARK: u32 = 0x80000000; // alpha simulado por cor sólida escura

// ============================================================================
// 2. Espaçamentos e Medidas
// ============================================================================

pub const SPACE_XS: usize = 4;
pub const SPACE_S: usize = 8;
pub const SPACE_M: usize = 16;
pub const SPACE_L: usize = 24;
pub const SPACE_XL: usize = 32;

pub const BORDER_RADIUS_S: usize = 4;
pub const BORDER_RADIUS_M: usize = 8;
pub const BORDER_RADIUS_L: usize = 12;

pub const BTN_HEIGHT: usize = 28;
pub const BTN_PAD_X: usize = 12;
pub const INPUT_HEIGHT: usize = 30;
pub const TOOLTIP_PAD: usize = 6;

// ============================================================================
// 3. Estados de Botão
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl ButtonState {
    pub fn bg_color(&self) -> u32 {
        match self {
            ButtonState::Normal => BG_SURFACE,
            ButtonState::Hover => ACCENT_HOVER,
            ButtonState::Pressed => ACCENT_PRESSED,
            ButtonState::Disabled => 0x001E293B,
        }
    }
    pub fn text_color(&self) -> u32 {
        match self {
            ButtonState::Disabled => 0x0064748B,
            _ => TEXT_PRIMARY,
        }
    }
}

// ============================================================================
// 4. Componente Tooltip
// ============================================================================

pub struct Tooltip {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

impl Tooltip {
    pub fn new(text: &str, x: i32, y: i32) -> Self {
        Self {
            text: String::from(text),
            x,
            y,
            visible: true,
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn show(&mut self) {
        self.visible = true;
    }
}

// ============================================================================
// 5. Componente Modal (caixa de confirmação / alerta)
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModalResult {
    None,
    Ok,
    Cancel,
    Yes,
    No,
}

pub struct Modal {
    pub title: String,
    pub message: String,
    pub result: ModalResult,
    pub visible: bool,
    pub focus_yes: bool, // qual botão tem foco (sim/não)
}

impl Modal {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: String::from(title),
            message: String::from(message),
            result: ModalResult::None,
            visible: false,
            focus_yes: true,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.result = ModalResult::None;
        self.focus_yes = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

// ============================================================================
// 6. Componente Toggle Switch
// ============================================================================

pub struct Toggle {
    pub checked: bool,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub label: String,
}

impl Toggle {
    pub fn new(x: usize, y: usize, label: &str) -> Self {
        Self {
            checked: false,
            x,
            y,
            w: 56,
            h: 28,
            label: String::from(label),
        }
    }

    pub fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    pub fn knob_x(&self) -> usize {
        if self.checked {
            self.x + self.w - self.h
        } else {
            self.x
        }
    }

    pub fn track_color(&self) -> u32 {
        if self.checked { ACCENT_PRIMARY } else { BORDER_SUBTLE }
    }

    pub fn knob_color(&self) -> u32 {
        TEXT_PRIMARY
    }
}

// ============================================================================
// 7. Componente Slider (volume, brilho, etc.)
// ============================================================================

pub struct Slider {
    pub value: f32, // 0.0 .. 1.0
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub label: String,
}

impl Slider {
    pub fn new(x: usize, y: usize, w: usize, label: &str) -> Self {
        Self {
            value: 0.5,
            x,
            y,
            w,
            h: 6,
            label: String::from(label),
        }
    }

    pub fn thumb_x(&self) -> usize {
        self.x + ((self.value * (self.w as f32)) as usize)
    }
}

// ============================================================================
// 8. Componente Dropdown / Seletor
// ============================================================================

pub struct Dropdown {
    pub selected_idx: usize,
    pub options: Vec<String>,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub expanded: bool,
}

impl Dropdown {
    pub fn new(x: usize, y: usize, w: usize, options: Vec<&str>) -> Self {
        Self {
            selected_idx: 0,
            options: options.iter().map(|s| String::from(*s)).collect(),
            x,
            y,
            w,
            h: INPUT_HEIGHT,
            expanded: false,
        }
    }

    pub fn selected(&self) -> &str {
        &self.options[self.selected_idx]
    }
}

// ============================================================================
// 9. Gerenciador de Foco (navegação por Tab)
// ============================================================================

pub struct FocusManager {
    pub focused_idx: usize,
    pub total_widgets: usize,
    pub wrap_around: bool,
}

impl FocusManager {
    pub fn new(total: usize) -> Self {
        Self {
            focused_idx: 0,
            total_widgets: total,
            wrap_around: true,
        }
    }

    pub fn next(&mut self) {
        if self.total_widgets == 0 {
            return;
        }
        self.focused_idx = if self.focused_idx + 1 < self.total_widgets {
            self.focused_idx + 1
        } else if self.wrap_around {
            0
        } else {
            self.focused_idx
        };
    }

    pub fn prev(&mut self) {
        if self.total_widgets == 0 {
            return;
        }
        self.focused_idx = if self.focused_idx > 0 {
            self.focused_idx - 1
        } else if self.wrap_around {
            self.total_widgets - 1
        } else {
            0
        };
    }

    pub fn set(&mut self, idx: usize) {
        if idx < self.total_widgets {
            self.focused_idx = idx;
        }
    }
}

// ============================================================================
// 10. Atalhos de Teclado (hotkeys)
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    None,
    AltTab,
    WinD,
    WinS,
    AltF4,
    Escape,
    CtrlC,
    CtrlV,
    CtrlX,
    CtrlZ,
    CtrlS,
    CtrlO,
    CtrlN,
    CtrlP,
    SuperTab, // alternar áreas de trabalho (futuro)
}

pub struct HotkeyRegistry {
    pub alt_pressed: bool,
    pub ctrl_pressed: bool,
    pub win_pressed: bool,
    pub shift_pressed: bool,
}

impl HotkeyRegistry {
    pub const fn new() -> Self {
        Self {
            alt_pressed: false,
            ctrl_pressed: false,
            win_pressed: false,
            shift_pressed: false,
        }
    }

    pub fn set_modifier(&mut self, key: u8, pressed: bool) {
        match key {
            0x38 => self.alt_pressed = pressed,       // LAlt
            0x1D => self.ctrl_pressed = pressed,      // LCtrl
            0x5B => self.win_pressed = pressed,       // LWin
            0x2A | 0x36 => self.shift_pressed = pressed, // LShift / RShift
            _ => {}
        }
    }

    pub fn check_hotkey(&self, scancode: u8) -> HotkeyAction {
        // Alt + Tab
        if self.alt_pressed && scancode == 0x0F {
            return HotkeyAction::AltTab;
        }
        // Win + D
        if self.win_pressed && scancode == 0x20 {
            return HotkeyAction::WinD;
        }
        // Win + S
        if self.win_pressed && scancode == 0x1F {
            return HotkeyAction::WinS;
        }
        // Alt + F4
        if self.alt_pressed && scancode == 0x3D {
            return HotkeyAction::AltF4;
        }
        // Escape
        if scancode == 0x01 {
            return HotkeyAction::Escape;
        }
        // Ctrl + C / V / X / Z / S / O / N / P
        if self.ctrl_pressed {
            match scancode {
                0x2E => return HotkeyAction::CtrlC,
                0x2F => return HotkeyAction::CtrlV,
                0x2D => return HotkeyAction::CtrlX,
                0x2C => return HotkeyAction::CtrlZ,
                0x1F => return HotkeyAction::CtrlS,
                0x18 => return HotkeyAction::CtrlO,
                0x31 => return HotkeyAction::CtrlN,
                0x19 => return HotkeyAction::CtrlP,
                _ => {}
            }
        }
        HotkeyAction::None
    }
}

// ============================================================================
// 11. Renderização de componentes base (declarativas para gfx2d)
// ============================================================================

pub fn draw_focus_ring(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    use crate::drivers::gfx2d::{draw_rounded_rect_border, fill_rounded_rect};
    // Desenha um anel de foco externo (2px)
    let pad = 2;
    let rx = x.saturating_sub(pad);
    let ry = y.saturating_sub(pad);
    let rw = w.saturating_add(pad * 2);
    let rh = h.saturating_add(pad * 2);
    unsafe {
        let _ = fill_rounded_rect(fb, pitch, scr_w, scr_h, rx, ry, rw, rh, BORDER_RADIUS_M, OVERLAY_DARK);
        let _ = draw_rounded_rect_border(fb, pitch, scr_w, scr_h, rx, ry, rw, rh, BORDER_RADIUS_M, color);
    }
}

/// Converte um número para string curta (sem alloc pesado)
pub fn u32_to_str(n: u32, buf: &mut [u8]) -> &str {
    if n == 0 {
        buf[0] = b'0';
        buf[1] = 0;
        return core::str::from_utf8(buf).unwrap_or("");
    }
    let mut len = 0;
    let mut tmp = n;
    while tmp > 0 {
        buf[len] = b'0' + (tmp % 10) as u8;
        tmp /= 10;
        len += 1;
    }
    // inverter
    let mut i = 0;
    let mut j = len - 1;
    while i < j {
        buf.swap(i, j);
        i += 1;
        if j == 0 { break; }
        j -= 1;
    }
    buf[len] = 0;
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}
