//! ux_global_search.rs — Busca Global Unificada (Win+S / Super+S)
//!
//! Busca em apps, arquivos VFS e configurações. Renderizado como sobreposição
//! sobre o framebuffer, sem dependência de GPU.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::sync::atomic::Ordering as AtomicOrdering;

use crate::drivers::desktop::{ALL_APPS, AppId, DESKTOP};
use crate::drivers::ux_design_system::{
    BG_SURFACE, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_MUTED, ACCENT_PRIMARY,
    ACCENT_HOVER, BORDER_FOCUSED, BORDER_RADIUS_M, SPACE_S, SPACE_M, BTN_HEIGHT,
};
use crate::drivers::gfx2d::{
    fill_rounded_rect, draw_rounded_rect_border, draw_text_clipped,
};

/// Categoria de resultado de busca
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchCategory {
    All,
    Apps,
    Files,
    Settings,
}

/// Um resultado de busca
#[derive(Clone)]
pub struct SearchResult {
    pub id: u64,
    pub category: SearchCategory,
    pub title: String,
    pub subtitle: String,
    pub action_app: Option<AppId>,
}

/// Estado da busca global
pub struct GlobalSearchState {
    pub open: bool,
    pub query: String,
    pub category: SearchCategory,
    pub results: Vec<SearchResult>,
    pub selected: usize,
}

impl GlobalSearchState {
    pub const fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            category: SearchCategory::All,
            results: Vec::new(),
            selected: 0,
        }
    }

    /// Executa a busca com a query atual
    pub fn search(&mut self) {
        self.results.clear();
        let q = self.query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return;
        }

        // 1. Buscar apps
        if self.category == SearchCategory::All || self.category == SearchCategory::Apps {
            for meta in ALL_APPS.iter() {
                let title_lower = meta.title.to_ascii_lowercase();
                let cli_lower = meta.cli_name.to_ascii_lowercase();
                let matches_alias = meta.aliases.iter().any(|a| a.to_ascii_lowercase().contains(&q));
                if title_lower.contains(&q) || cli_lower.contains(&q) || matches_alias {
                    self.results.push(SearchResult {
                        id: meta.id as u64,
                        category: SearchCategory::Apps,
                        title: meta.title.to_string(),
                        subtitle: format!("App — {}", meta.cli_name),
                        action_app: Some(meta.id),
                    });
                }
            }
        }

        // 2. Buscar arquivos VFS (placeholder — integra com vfs_readdir)
        if self.category == SearchCategory::All || self.category == SearchCategory::Files {
            // Integração real com VFS será feita quando o VFS estiver exposto para busca recursiva
        }

        // 3. Buscar configurações (chaves do system.toml)
        if self.category == SearchCategory::All || self.category == SearchCategory::Settings {
            let settings_keys = [
                ("display.refresh_rate", "Taxa de atualização"),
                ("audio.bit_perfect", "Áudio bit-perfeito"),
                ("power.profile", "Perfil de energia"),
                ("network.dns_encrypted", "DNS criptografado"),
                ("privacy.zero_telemetry", "Telemetria zero"),
                ("update.channel_selection", "Canal de atualizações"),
            ];
            for (key, desc) in settings_keys.iter() {
                if key.contains(&q) || desc.to_ascii_lowercase().contains(&q) {
                    self.results.push(SearchResult {
                        id: key.as_ptr() as u64,
                        category: SearchCategory::Settings,
                        title: desc.to_string(),
                        subtitle: format!("Configuração — {}", key),
                        action_app: None,
                    });
                }
            }
        }

        // Ordenar: exato > prefixo > contém
        self.results.sort_by(|a, b| {
            let a_exact = a.title.to_ascii_lowercase() == q;
            let b_exact = b.title.to_ascii_lowercase() == q;
            let a_prefix = a.title.to_ascii_lowercase().starts_with(&q);
            let b_prefix = b.title.to_ascii_lowercase().starts_with(&q);
            match (a_exact, b_exact) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => match (a_prefix, b_prefix) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => Ordering::Equal,
                },
            }
        });

        if self.selected >= self.results.len() {
            self.selected = 0;
        }
    }

    /// Adiciona um caractere à query e reexecuta a busca
    pub fn push_char(&mut self, ch: char) {
        if !ch.is_control() && self.query.len() < 64 {
            self.query.push(ch);
            self.search();
            self.selected = 0;
        }
    }

    /// Remove o último caractere (backspace)
    pub fn backspace(&mut self) {
        self.query.pop();
        self.search();
        self.selected = 0;
    }

    /// Move a seleção para cima/baixo
    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let new_idx = (self.selected as isize + delta).rem_euclid(self.results.len() as isize) as usize;
        self.selected = new_idx;
    }

    /// Retorna o resultado selecionado, se houver
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }

    /// Limpa a busca
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected = 0;
    }
}

pub static GLOBAL_SEARCH: crate::sync::SpinLock<Option<GlobalSearchState>> =
    crate::sync::SpinLock::new(None);

/// Renderiza a sobreposição de busca global no centro da tela
pub unsafe fn render_global_search(
    fb: *mut u8,
    pitch: usize,
    scr_w: usize,
    scr_h: usize,
) {
    let binding = GLOBAL_SEARCH.lock();
    let search = match *binding {
        Some(ref s) => s,
        None => return,
    };
    if !search.open {
        return;
    }

    let overlay_w = 600;
    let overlay_h = 420;
    let ox = (scr_w / 2).saturating_sub(overlay_w / 2);
    let oy = (scr_h / 2).saturating_sub(overlay_h / 2);

    // Fundo escuro da sobreposição
    fill_rounded_rect(fb, pitch, scr_w, scr_h, ox, oy, overlay_w, overlay_h, BORDER_RADIUS_M, BG_SURFACE);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, ox, oy, overlay_w, overlay_h, BORDER_RADIUS_M, BORDER_FOCUSED);

    // Título
    draw_text_clipped(fb, pitch, scr_w, scr_h, ox + SPACE_M, oy + SPACE_S, ox + overlay_w - SPACE_M, "Busca Global", TEXT_PRIMARY, None);

    // Barra de busca
    let bar_y = oy + SPACE_M + 24;
    let bar_h = BTN_HEIGHT;
    fill_rounded_rect(fb, pitch, scr_w, scr_h, ox + SPACE_S, bar_y, overlay_w - SPACE_S * 2, bar_h, BORDER_RADIUS_M, 0x001E293B);
    draw_rounded_rect_border(fb, pitch, scr_w, scr_h, ox + SPACE_S, bar_y, overlay_w - SPACE_S * 2, bar_h, BORDER_RADIUS_M, BORDER_FOCUSED);

    let query_display = format!("🔍 {}", search.query);
    draw_text_clipped(fb, pitch, scr_w, scr_h, ox + SPACE_S + 10, bar_y + 4, ox + overlay_w - SPACE_S, &query_display, TEXT_PRIMARY, None);

    // Cursor piscante simples
    let ticks = crate::arch::idt::SYSTEM_TICKS.load(AtomicOrdering::Relaxed);
    if (ticks / 10) % 2 == 0 {
        let cursor_x = ox + SPACE_S + 10 + (search.query.len() * 8) + 10;
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cursor_x, bar_y + 6, 2, bar_h - 12, 1, TEXT_PRIMARY);
    }

    // Chips de filtro
    let chip_y = bar_y + bar_h + SPACE_S;
    let chips = ["Todos", "Apps", "Arquivos", "Config"];
    for (i, chip) in chips.iter().enumerate() {
        let cx = ox + SPACE_S + i * 70;
        let cw = 64;
        let active = match *chip {
            "Todos" => search.category == SearchCategory::All,
            "Apps" => search.category == SearchCategory::Apps,
            "Arquivos" => search.category == SearchCategory::Files,
            "Config" => search.category == SearchCategory::Settings,
            _ => false,
        };
        fill_rounded_rect(fb, pitch, scr_w, scr_h, cx, chip_y, cw, 24, BORDER_RADIUS_M, if active { ACCENT_PRIMARY } else { 0x001E293B });
        draw_text_clipped(fb, pitch, scr_w, scr_h, cx + 8, chip_y + 4, cx + cw, chip, TEXT_PRIMARY, None);
    }

    // Lista de resultados
    let list_y = chip_y + 32;
    let list_h = overlay_h - (list_y - oy) - SPACE_M;
    let max_items = (list_h / 40) as usize;
    let results = &search.results;

    if results.is_empty() {
        draw_text_clipped(fb, pitch, scr_w, scr_h, ox + SPACE_M, list_y + 10, ox + overlay_w - SPACE_M, "Nenhum resultado encontrado", TEXT_MUTED, None);
    } else {
        for (i, result) in results.iter().enumerate().take(max_items) {
            let ry = list_y + i * 40;
            let selected = i == search.selected;
            if selected {
                fill_rounded_rect(fb, pitch, scr_w, scr_h, ox + SPACE_S, ry, overlay_w - SPACE_S * 2, 36, BORDER_RADIUS_M, ACCENT_PRIMARY);
            }
            let icon_x = ox + SPACE_S + 10;
            let title_x = ox + SPACE_S + 42;
            let icon_color = if selected { TEXT_PRIMARY } else { TEXT_SECONDARY };
            let title_color = if selected { TEXT_PRIMARY } else { TEXT_PRIMARY };
            let sub_color = if selected { TEXT_PRIMARY } else { TEXT_MUTED };

            // Ícone simples por categoria
            let icon_glyph = match result.category {
                SearchCategory::Apps => "[A]",
                SearchCategory::Files => "[F]",
                SearchCategory::Settings => "[S]",
                SearchCategory::All => "[•]",
            };
            draw_text_clipped(fb, pitch, scr_w, scr_h, icon_x, ry + 8, icon_x + 24, icon_glyph, icon_color, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, title_x, ry + 6, ox + overlay_w - SPACE_S, &result.title, title_color, None);
            draw_text_clipped(fb, pitch, scr_w, scr_h, title_x, ry + 22, ox + overlay_w - SPACE_S, &result.subtitle, sub_color, None);
        }
        if results.len() > max_items {
            draw_text_clipped(fb, pitch, scr_w, scr_h, ox + SPACE_M, oy + overlay_h - SPACE_S - 12, ox + overlay_w - SPACE_M, &format!("+ {} mais", results.len() - max_items), TEXT_MUTED, None);
        }
    }

    // Instruções
    draw_text_clipped(fb, pitch, scr_w, scr_h, ox + SPACE_M, oy + overlay_h - SPACE_S, ox + overlay_w - SPACE_M, "↑↓ navegar · Enter abrir · Esc fechar", TEXT_MUTED, None);
}

/// Executa a ação do resultado selecionado (abre app ou config)
pub fn execute_selected() {
    let binding = GLOBAL_SEARCH.lock();
    let search = match *binding {
        Some(ref s) => s,
        None => return,
    };
    if let Some(result) = search.selected_result() {
        if let Some(app_id) = result.action_app {
            drop(binding);
            let mut desk = DESKTOP.lock();
            if let Some(ref mut d) = *desk {
                d.open_window(app_id);
                d.start_menu_open = false;
            }
        }
    }
}

/// Fecha a busca
pub fn close_search() {
    let mut s = GLOBAL_SEARCH.lock();
    if let Some(ref mut search) = *s {
        search.open = false;
        search.clear();
    }
}

/// Alterna a busca global
pub fn toggle_search() {
    let mut s = GLOBAL_SEARCH.lock();
    if let Some(ref mut search) = *s {
        search.open = !search.open;
        if search.open {
            search.search();
        } else {
            search.clear();
        }
    }
}
