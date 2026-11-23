//! ux_notifications.rs — Central de Notificações e Toast
//!
//! Sistema de notificações com ícone de sino, badge, painel deslizante e
//! toasts temporários. Todo renderizado no framebuffer via software.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::Ordering as AtomicOrdering;

use crate::drivers::ux_design_system::{
    BG_SURFACE, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_MUTED, ACCENT_PRIMARY,
    ERROR_COLOR, BORDER_FOCUSED, BORDER_RADIUS_M, SPACE_S, SPACE_M, BTN_HEIGHT,
};
use crate::drivers::gfx2d::{
    fill_rounded_rect, draw_rounded_rect_border, draw_text_clipped,
};

/// Uma notificação individual
#[derive(Clone)]
pub struct Notification {
    pub id: u64,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_secs: u64,
    pub dismissed: bool,
    pub toast: bool,
    pub expires_at: Option<u64>,
}

/// Estado da central de notificações
pub struct NotificationCenter {
    pub notifications: Vec<Notification>,
    pub open: bool,
    pub slide_offset: i32, // -largura (fechado) .. 0 (aberto)
    pub next_id: u64,
    pub unread_count: u32,
}

impl NotificationCenter {
    pub const fn new() -> Self {
        Self {
            notifications: Vec::new(),
            open: false,
            slide_offset: -400,
            next_id: 1,
            unread_count: 0,
        }
    }

    /// Adiciona uma notificação normal
    pub fn notify(&mut self, app_name: &str, title: &str, body: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(Notification {
            id,
            app_name: String::from(app_name),
            title: String::from(title),
            body: String::from(body),
            timestamp_secs: crate::arch::idt::SYSTEM_TICKS.load(AtomicOrdering::Relaxed) / 100,
            dismissed: false,
            toast: false,
            expires_at: None,
        });
        self.unread_count += 1;
    }

    /// Adiciona um toast temporário (some sozinho)
    pub fn toast(&mut self, title: &str, body: &str, duration_secs: u64) {
        let now = crate::arch::idt::SYSTEM_TICKS.load(AtomicOrdering::Relaxed) / 100;
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.push(Notification {
            id,
            app_name: String::from("Sistema"),
            title: String::from(title),
            body: String::from(body),
            timestamp_secs: now,
            dismissed: false,
            toast: true,
            expires_at: Some(now + duration_secs),
        });
    }

    /// Descarta uma notificação por ID
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id && !n.dismissed) {
            n.dismissed = true;
            if self.unread_count > 0 {
                self.unread_count -= 1;
            }
        }
    }

    /// Remove toasts expirados
    pub fn expire_toasts(&mut self) {
        let now = crate::arch::idt::SYSTEM_TICKS.load(AtomicOrdering::Relaxed) / 100;
        self.notifications.retain(|n| {
            if n.toast {
                match n.expires_at {
                    Some(exp) => exp > now,
                    None => true,
                }
            } else {
                true
            }
        });
    }

    /// Conta notificações não lidas (não-dismissed, não-toast)
    pub fn count_unread(&self) -> u32 {
        self.notifications.iter().filter(|n| !n.dismissed && !n.toast).count() as u32
    }
}

pub static NOTIFICATIONS: crate::sync::SpinLock<Option<NotificationCenter>> =
    crate::sync::SpinLock::new(None);

/// Renderiza o badge de notificação sobre o ícone de sino na taskbar
pub unsafe fn render_notification_badge(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
    bell_x: i32,
    bell_y: i32,
) {
    let binding = NOTIFICATIONS.lock();
    let n = match *binding {
        Some(ref n) => n,
        None => return,
    };
    let unread = n.count_unread();
    if unread == 0 {
        return;
    }

    // Badge vermelho no canto superior direito do sino
    let bx = bell_x + 18;
    let by = bell_y - 6;
    let r = 6i32;
    let d = r * 2;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, bx as usize, by as usize, d as usize, d as usize, r as usize, ERROR_COLOR);
    let count_str = if unread > 9 { "9+".to_string() } else { unread.to_string() };
    draw_text_clipped(fb, pitch, scr_w, scr_h, bx as usize + 2, by as usize + 1, (bx + d) as usize, &count_str, TEXT_PRIMARY, None);
}

/// Renderiza o painel deslizante de notificações (lado direito da tela)
pub unsafe fn render_notification_panel(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
) {
    let binding = NOTIFICATIONS.lock();
    let n = match *binding {
        Some(ref n) => n,
        None => return,
    };
    if !n.open {
        return;
    }

    let panel_w = 360;
    let panel_x = (scr_w as i32 + n.slide_offset).max(0) as usize;
    let panel_y = 60;
    let panel_h = scr_h - panel_y - 60;

    // Fundo do painel
    fill_rounded_rect(fb, pitch, scr_w, scr_h, panel_x, panel_y, panel_w, panel_h, BORDER_RADIUS_M, BG_SURFACE);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, panel_x, panel_y, panel_w, panel_h, BORDER_RADIUS_M, BORDER_FOCUSED);

    // Título
    draw_text_clipped(fb, pitch, scr_w, scr_h, panel_x + SPACE_M, panel_y + SPACE_S, panel_x + panel_w - SPACE_M, "Notificações", TEXT_PRIMARY, None);

    // Botão limpar todas
    let clear_x = panel_x + panel_w - SPACE_M - 80;
    let clear_y = panel_y + SPACE_S;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, clear_x, clear_y, 76, 24, BORDER_RADIUS_M, 0x001E293B);
    draw_text_clipped(fb, pitch, scr_w, scr_h, clear_x + 8, clear_y + 4, clear_x + 76, "Limpar tudo", TEXT_SECONDARY, None);

    // Lista de notificações
    let mut y = panel_y + SPACE_M + 32;
    let item_h = 64;
    let visible = &n.notifications;
    let max_items = ((panel_h - SPACE_M - 40) / item_h) as usize;

    if visible.is_empty() || visible.iter().all(|n| n.dismissed) {
        draw_text_clipped(fb, pitch, scr_w, scr_h, panel_x + SPACE_M, y, panel_x + panel_w - SPACE_M, "Sem notificações", TEXT_MUTED, None);
    } else {
        let shown = visible.iter().rev().take(max_items);
        for notif in shown {
            if notif.dismissed {
                continue;
            }
            let item_x = panel_x + SPACE_S;
            let item_w = panel_w - SPACE_S * 2;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, item_x, y, item_w, item_h, BORDER_RADIUS_M, 0x001E293B);

            // Título
            draw_text_clipped(fb, pitch, scr_w, scr_h, item_x + 42, y + 10, item_x + item_w - 40, &notif.title, TEXT_PRIMARY, None);
            // App
            draw_text_clipped(fb, pitch, scr_w, scr_h, item_x + 42, y + 26, item_x + item_w - 60, &notif.app_name, TEXT_MUTED, None);
            // Corpo (truncado)
            let body_max = if notif.body.len() > 60 { &notif.body[..60] } else { &notif.body };
            draw_text_clipped(fb, pitch, scr_w, scr_h, item_x + 42, y + 42, item_x + item_w - 40, body_max, TEXT_SECONDARY, None);

            // Botão X descartar
            let x_x = item_x + item_w - 28;
            fill_rounded_rect(fb, pitch, scr_w, scr_h, x_x, y + 16, 20, 20, BORDER_RADIUS_M, 0x00334155);
            draw_text_clipped(fb, pitch, scr_w, scr_h, x_x + 6, y + 20, x_x + 20, "×", TEXT_PRIMARY, None);

            y += item_h + SPACE_S;
        }
    }

    // Instrução
    draw_text_clipped(fb, pitch, scr_w, scr_h, panel_x + SPACE_M, panel_y + panel_h - SPACE_S, panel_x + panel_w - SPACE_M, "Clique X para descartar · Clique fora para fechar", TEXT_MUTED, None);
}

/// Renderiza um toast no canto inferior direito
pub unsafe fn render_toasts(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
) {
    let binding = NOTIFICATIONS.lock();
    let n = match *binding {
        Some(ref n) => n,
        None => return,
    };

    let mut y = (scr_h as i32 - 80) as usize;
    let toast_w = 300;
    let toast_h = 64;
    let toast_x = scr_w - toast_w - SPACE_S;

    for notif in n.notifications.iter() {
        if !notif.toast || notif.dismissed {
            continue;
        }
        // Verificar expiração
        let now = crate::arch::idt::SYSTEM_TICKS.load(AtomicOrdering::Relaxed) / 100;
        if let Some(exp) = notif.expires_at {
            if exp <= now {
                continue;
            }
        }

        fill_rounded_rect(fb, pitch, scr_w, scr_h, toast_x, y, toast_w, toast_h, BORDER_RADIUS_M, BG_SURFACE);
        draw_rounded_rect_border(fb, pitch, scr_w, scr_h, toast_x, y, toast_w, toast_h, BORDER_RADIUS_M, ACCENT_PRIMARY);

        draw_text_clipped(fb, pitch, scr_w, scr_h, toast_x + 12, y + 8, toast_x + toast_w - 12, &notif.title, TEXT_PRIMARY, None);
        draw_text_clipped(fb, pitch, scr_w, scr_h, toast_x + 12, y + 26, toast_x + toast_w - 12, &notif.body, TEXT_SECONDARY, None);

        y = y.saturating_sub(toast_h + SPACE_S);
    }
}

/// Fecha o painel de notificações
pub fn close_panel() {
    let mut n = NOTIFICATIONS.lock();
    if let Some(ref mut center) = *n {
        center.open = false;
        center.slide_offset = -400;
    }
}

/// Alterna o painel de notificações
pub fn toggle_panel() {
    let mut n = NOTIFICATIONS.lock();
    if let Some(ref mut center) = *n {
        center.open = !center.open;
        center.slide_offset = if center.open { 0 } else { -400 };
    }
}
