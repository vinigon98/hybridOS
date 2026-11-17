//! hyb-settings — Central Unificada de Configurações do Hybridos OS.
//! Interface (texto) dos Módulos 1 a 5 da especificação.
//! Ring 3, #![no_std], #![no_main], alvo x86_64-unknown-none.
//! Lê /etc/hybridos/system.toml e exibe os parâmetros dos módulos.
//! Argumentos lidos de etc/hybridos/hyb-settings.args (ex.: "view power" ou "list").

#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::println;
use libhybrid_sys::linux::{sys_close, sys_exit, sys_open, sys_read, sys_write};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-settings] erro interno inesperado");
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

const CONFIG_PATH: &str = "etc/hybridos/system.toml";
const ARGS_PATH: &str = "etc/hybridos/hyb-settings.args";
const O_RDONLY: u32 = 0;
const MODE_0644: u32 = 0o644;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let args_text = read_args();
    let words: alloc::vec::Vec<&str> = args_text.split_whitespace().collect();
    match words.get(0).copied() {
        Some("view") | Some("show") => {
            if let Some(m) = words.get(1).copied() {
                cmd_view(m);
            } else {
                eprintln_bare("Uso: hyb-settings view <modulo>");
                sys_exit(2);
            }
        }
        Some("list") | None => {
            cmd_overview();
        }
        other => {
            eprintln_bare(&format!("hyb-settings: comando desconhecido: {:?}", other));
            print_help();
            sys_exit(2);
        }
    }
    sys_exit(0);
}

// ---------------------------------------------------------------------------
// Argumentos (arquivo etc/hybridos/hyb-settings.args)
// ---------------------------------------------------------------------------

fn read_args() -> alloc::string::String {
    match read_file(ARGS_PATH) {
        Some(bytes) => match alloc::string::String::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => alloc::string::String::new(),
        },
        None => alloc::string::String::new(),
    }
}

// ---------------------------------------------------------------------------
// Leitura de arquivo (syscalls Linux)
// ---------------------------------------------------------------------------

fn open_readonly(path: &str) -> Option<usize> {
    let fd = sys_open(path, O_RDONLY, 0);
    if fd < 0 {
        return None;
    }
    Some(fd as usize)
}

fn read_file(path: &str) -> Option<alloc::vec::Vec<u8>> {
    let fd = match open_readonly(path) {
        Some(f) => f,
        None => return None,
    };
    let mut out: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd);
    Some(out)
}

// ---------------------------------------------------------------------------
// Valor TOML
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum TomlValue {
    Bool(bool),
    Integer(i64),
    Str(alloc::string::String),
}

impl TomlValue {
    fn parse(s: &str) -> TomlValue {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("true") {
            return TomlValue::Bool(true);
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return TomlValue::Bool(false);
        }
        if let Ok(n) = trimmed.parse::<i64>() {
            return TomlValue::Integer(n);
        }
        let stripped = trimmed.strip_prefix('"').and_then(|inner| inner.strip_suffix('"'));
        match stripped {
            Some(inner) => TomlValue::Str(unescape_toml_string(inner)),
            None => TomlValue::Str(alloc::string::String::from(trimmed)),
        }
    }

    fn display(&self) -> alloc::string::String {
        match self {
            TomlValue::Bool(b) => format!("{}", b),
            TomlValue::Integer(n) => format!("{}", n),
            TomlValue::Str(s) => s.clone(),
        }
    }
}

fn unescape_toml_string(s: &str) -> alloc::string::String {
    let mut out = alloc::string::String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(match next {
                    'n' => '\n', 'r' => '\r', 't' => '\t', '"' => '"', '\\' => '\\', c => c,
                });
            }
        } else {
            out.push(ch);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

struct Entry {
    key: alloc::string::String,
    value: TomlValue,
}

struct Section {
    name: alloc::string::String,
    entries: alloc::vec::Vec<Entry>,
}

struct Config {
    sections: alloc::vec::Vec<Section>,
}

impl Config {
    fn new() -> Config {
        Config { sections: alloc::vec::Vec::new() }
    }
    fn section_index(&self, name: &str) -> Option<usize> {
        let mut i = 0;
        while i < self.sections.len() {
            if self.sections[i].name == name {
                return Some(i);
            }
            i += 1;
        }
        None
    }
    fn get(&self, full_key: &str) -> Option<&TomlValue> {
        let pos = match full_key.rfind('.') {
            Some(p) if p > 0 && p < full_key.len() - 1 => p,
            _ => return None,
        };
        let (section, key) = (&full_key[..pos], &full_key[pos + 1..]);
        let idx = self.section_index(section)?;
        let entry = self.sections[idx].entries.iter().find(|e| e.key == key)?;
        Some(&entry.value)
    }
}

fn parse_toml(text: &str) -> Config {
    let mut cfg = Config::new();
    let mut current_section: Option<alloc::string::String> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')) {
            let name = rest.trim();
            if !name.is_empty() {
                if cfg.section_index(name).is_none() {
                    cfg.sections.push(Section {
                        name: alloc::string::String::from(name),
                        entries: alloc::vec::Vec::new(),
                    });
                }
                current_section = Some(alloc::string::String::from(name));
            }
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let mut vt = &line[eq_pos + 1..];
            vt = strip_inline_comment(vt);
            if key.is_empty() {
                continue;
            }
            let value = TomlValue::parse(vt);
            let section_name = current_section.clone().unwrap_or_else(|| alloc::string::String::from("default"));
            if cfg.section_index(&section_name).is_none() {
                cfg.sections.push(Section {
                    name: section_name.clone(),
                    entries: alloc::vec::Vec::new(),
                });
            }
            let idx = cfg.section_index(&section_name).unwrap();
            let entry = Entry { key: alloc::string::String::from(key), value };
            match cfg.sections[idx].entries.iter().position(|e| e.key == key) {
                Some(pos) => cfg.sections[idx].entries[pos] = entry,
                None => cfg.sections[idx].entries.push(entry),
            }
        }
    }
    cfg
}

fn strip_inline_comment(s: &str) -> &str {
    let mut in_quotes = false;
    for (i, ch) in s.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return s[..i].trim(),
            _ => {}
        }
    }
    s
}

// ---------------------------------------------------------------------------
// E/S
// ---------------------------------------------------------------------------

fn eprintln_bare(text: &str) {
    let _ = sys_write(2, text.as_bytes());
    let _ = sys_write(2, b"\n");
}

fn print_help() {
    println!("hyb-settings — Central Unificada de Configurações do Hybridos OS");
    println!("Módulos: 1 display 2 audio 3 input 4 power 5 shell");
    println!("Uso (escreva o comando em etc/hybridos/hyb-settings.args):");
    println!("  hyb-settings list        — visão geral dos 5 módulos");
    println!("  hyb-settings view <mod>  — detalhes de um módulo (ex.: view power)");
    println!();
    println!("Exemplos:");
    println!("  view display");
    println!("  view power");
    println!("  view shell");
}

fn cmd_overview() {
    let text = match read_file(CONFIG_PATH) {
        Some(t) => t,
        None => {
            eprintln_bare(&format!("hyb-settings: não foi possível ler {}", CONFIG_PATH));
            sys_exit(1);
        }
    };
    let text = match alloc::string::String::from_utf8(text) {
        Ok(t) => t,
        Err(_) => alloc::string::String::new(),
    };
    let cfg = parse_toml(&text);

    let modules: &[(&str, &str, &[&str])] = &[
        ("display", "Módulo 1 — Compositor de Vídeo", &["refresh_rate", "scaling", "multi_monitor", "night_light", "hdr_pipeline", "disable_cabc"]),
        ("graphics", "Módulo 1 — Pipeline Gráfico", &["scheduler_hags", "direct_scanout", "gpu_preference", "tdr_recovery", "legacy_dpi_fix", "frame_limiter_global"]),
        ("audio", "Módulo 2 — Áudio", &["devices_profile", "bit_perfect", "quantum_buffer", "lock_mic_gain", "ducking", "rnnoise_filter", "per_app_routing", "system_sounds"]),
        ("input.mouse", "Módulo 3 — Mouse", &["raw_input", "polling_rate_support", "scroll_inactive", "middle_click_paste"]),
        ("input.keyboard", "Módulo 3 — Teclado", &["repeat_rate", "no_sticky_keys", "remap_caps", "clean_layout"]),
        ("input.touchpad", "Módulo 3 — Touchpad", &["gestures"]),
        ("input.usb", "Módulo 3 — USB", &["no_autosuspend"]),
        ("power", "Módulo 4 — Energia", &["profile", "pcie_aspm", "cpu_scheduler", "boost_control", "clean_shutdown", "hibernation", "battery_charge_limit", "disk_spindown"]),
        ("files", "Módulo 5 — Explorador", &["show_extensions", "show_dotfiles", "default_view", "zero_ads", "header_path"]),
        ("shell", "Módulo 5 — Shell", &["context_menu", "taskbar_clean", "panel_position", "force_kill_shortcut", "launcher", "window_tiling", "virtual_desktops", "compositor_effects", "tray_telemetry"]),
    ];

    let mut out = alloc::string::String::new();
    out.push_str("================================================================\n");
    out.push_str(" hyb-settings — Central Unificada de Configurações\n");
    out.push_str(" Módulos 1 a 5 — Agente 6\n");
    out.push_str("================================================================\n\n");
    let mut total = 0usize;
    for (sec, title, keys) in modules.iter() {
        let idx = match cfg.section_index(sec) {
            Some(i) => i,
            None => continue,
        };
        let section = &cfg.sections[idx];
        let mut shown = 0usize;
        for k in keys.iter() {
            if let Some(pos) = section.entries.iter().position(|e| e.key == *k) {
                out.push_str(&format!("  {}.{} = {}\n", sec, k, section.entries[pos].value.display()));
                shown += 1;
            }
        }
        if shown > 0 {
            out.push_str(&format!("[{}] {} — {}/{} parâmetros\n\n", sec, title, shown, keys.len()));
            total += shown;
        }
    }
    out.push_str(&format!("Total de parâmetros exibidos (Módulos 1 a 5): {}\n", total));
    out.push_str("\nUse: hyb-settings view <modulo>  (ex.: view power)\n");
    let _ = sys_write(1, out.as_bytes());
}

fn cmd_view(module: &str) {
    let text = match read_file(CONFIG_PATH) {
        Some(t) => t,
        None => {
            eprintln_bare(&format!("hyb-settings: não foi possível ler {}", CONFIG_PATH));
            sys_exit(1);
        }
    };
    let text = match alloc::string::String::from_utf8(text) {
        Ok(t) => t,
        Err(_) => alloc::string::String::new(),
    };
    let cfg = parse_toml(&text);

    let idx = match cfg.section_index(module) {
        Some(i) => i,
        None => {
            eprintln_bare(&format!("hyb-settings: módulo não encontrado: {}", module));
            sys_exit(1);
        }
    };
    let section = &cfg.sections[idx];

    let mut out = alloc::string::String::new();
    out.push_str(&format!("\n================================================================\n"));
    out.push_str(&format!(" Módulo [{}] — Detalhes\n", section.name));
    out.push_str(&format!("================================================================\n\n"));
    for entry in &section.entries {
        out.push_str(&format!("  {} = {}\n", entry.key, entry.value.display()));
    }
    out.push_str(&format!("\nTotal: {} parâmetro(s) em [{}]\n", section.entries.len(), section.name));
    out.push_str("\nVoltar: hyb-settings list\n");
    let _ = sys_write(1, out.as_bytes());
}
