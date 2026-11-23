//! hyb-settings: Central Unificada de Configurações (Super + I).
//! Interface dos módulos de configuração 6 a 10 do Hybridos OS.
//! Lê /etc/hybridos/system.toml e exibe os parâmetros dos módulos 6-10.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit};

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-settings] erro interno inesperado");
    sys_exit(1);
}

const CONFIG_PATHS: [&str; 2] = ["etc/hybridos/system.toml", "/etc/hybridos/system.toml"];
const ARGS_PATHS: [&str; 2] = ["etc/hybridos/hybctl.args", "/etc/hybridos/hybctl.args"];
const O_RDONLY: u32 = 0;
const O_WRONLY: u32 = 1;
const O_CREAT: u32 = 0x40;
const O_TRUNC: u32 = 0x200;

#[derive(Debug, Clone, PartialEq)]
enum TomlValue { Str(String), Bool(bool), Int(i64) }
#[derive(Debug, Clone)]
struct TomlEntry { section: String, key: String, value: TomlValue }
struct TomlDoc { entries: Vec<TomlEntry>, sections: Vec<String> }
impl TomlDoc {
    fn new() -> Self { Self { entries: Vec::new(), sections: Vec::new() } }
    fn ensure_section(&mut self, section: &str) {
        if !self.sections.iter().any(|s| s == section) { self.sections.push(String::from(section)); }
    }
    fn set(&mut self, section: &str, key: &str, value: TomlValue) {
        self.ensure_section(section);
        for e in self.entries.iter_mut() { if e.section == section && e.key == key { e.value = value; return; } }
        self.entries.push(TomlEntry { section: String::from(section), key: String::from(key), value });
    }
    fn get(&self, section: &str, key: &str) -> Option<&TomlValue> {
        self.entries.iter().find(|e| e.section == section && e.key == key).map(|e| &e.value)
    }
}
fn trim(s: &str) -> &str {
    let st = s.find(|c: char| !c.is_whitespace()).unwrap_or(0);
    let en = s.rfind(|c: char| !c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    if en <= st { "" } else { &s[st..en] }
}
fn strip_comment(line: &str) -> &str {
    let mut q = false; let mut cut = line.len();
    for (i, c) in line.char_indices() { if c == '"' { q = !q; } else if c == '#' && !q { cut = i; break; } }
    &line[..cut]
}
fn parse_value(raw: &str) -> TomlValue {
    let s = trim(raw);
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') { return TomlValue::Str(String::from(&s[1..s.len()-1])); }
    match s { "true" => TomlValue::Bool(true), "false" => TomlValue::Bool(false), _ => {
        if let Ok(n) = s.parse::<i64>() { TomlValue::Int(n) } else { TomlValue::Str(String::from(s)) }
    } }
}
fn open_readonly(path: &str) -> Option<usize> { let fd = sys_open(path, O_RDONLY, 0); if fd < 0 { return None; } Some(fd as usize) }
fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match open_readonly(path) { Some(f) => f, None => return None };
    let mut out = Vec::new(); let mut buf = [0u8; 4096];
    loop { let n = sys_read(fd, &mut buf); if n <= 0 { break; } out.extend_from_slice(&buf[..n as usize]); }
    let _ = sys_close(fd); Some(out)
}
fn load_doc() -> Option<TomlDoc> {
    for path in CONFIG_PATHS.iter() {
        if let Some(data) = read_file(path) {
            let text = match core::str::from_utf8(&data) { Ok(t) => t, Err(_) => continue };
            let mut doc = TomlDoc::new(); let mut cur: Option<String> = None;
            for line in text.lines() {
                let line = trim(strip_comment(line));
                if line.is_empty() { continue; }
                if line.starts_with('[') && line.ends_with(']') {
                    let sec = trim(&line[1..line.len()-1]);
                    if !sec.is_empty() { cur = Some(String::from(sec)); doc.ensure_section(sec); }
                    continue;
                }
                if let Some(eq) = line.find('=') {
                    let key = trim(&line[..eq]); let val = parse_value(&line[eq+1..]);
                    if key.is_empty() { continue; }
                    match &cur { Some(s) => doc.set(s, key, val), None => doc.set("__root__", key, val); }
                }
            }
            return Some(doc);
        }
    }
    None
}
fn format_value(v: &TomlValue) -> String {
    match v { TomlValue::Str(s) => format!("\"{}\"", s), TomlValue::Bool(b) => format!("{}", b), TomlValue::Int(n) => format!("{}", n) }
}
fn split_key(key: &str) -> Option<(&str, &str)> { let p = key.find('.')?; Some((&key[..p], &key[p+1..])) }
fn read_args_file() -> Option<Vec<String>> {
    for path in ARGS_PATHS.iter() {
        if let Some(data) = read_file(path) {
            let text = trim(core::str::from_utf8(&data).ok()?);
            if text.is_empty() { return None; }
            let mut args = Vec::new();
            for t in text.split_whitespace() { args.push(String::from(t)); }
            return Some(args);
        }
    }
    None
}
fn open_write(path: &str) -> Option<usize> { let fd = sys_open(path, O_WRONLY | O_CREAT | O_TRUNC, 0o644); if fd < 0 { return None; } Some(fd as usize) }
fn write_file(path: &str, contents: &str) -> bool {
    let fd = match open_write(path) { Some(f) => f, None => return false };
    let bytes = contents.as_bytes(); let mut w = 0usize;
    while w < bytes.len() { let n = sys_write(fd, &bytes[w..]); if n <= 0 { let _ = sys_close(fd); return false; } w += n as usize; }
    let _ = sys_close(fd); true
}

struct Modulo { numero: usize, nome: &'static str, descricao: &'static str,
    parametros: &'static [(&'static str, &'static str)] }

const MODULOS_6_10: [Modulo; 5] = [
    Modulo { numero: 6, nome: "Dev / FS / Memória", descricao: "Ambiente de desenvolvimento, sistema de arquivos e memória",
        parametros: &[
            ("dev.mode_default","Modo de desenvolvedor padrão"),("fs.unlimited_paths","Caminhos ilimitados (>260 chars)"),
            ("fs.case_sensitivity_per_dir","Case-sensitividade por diretório"),("fs.dev_optimization","Otimização para pastas de dev"),
            ("security.dev_trust_folders","Pastas de confiança do dev"),("locale.utf8_everywhere","UTF-8 em toda parte"),
            ("locale.technical_formats","Formatos técnicos independentes"),("env.session_reload","Recarregamento de sessão de env"),
            ("memory.zram_swap","ZRAM swap comprimida"),("virt.dynamic_memory","Memória dinâmica de VM/container"),
            ("virt.port_auto_bind","Auto-bind de portas localhost"),("dev.localhost_domains","Domínios *.localhost/*.test"),
            ("storage.temp_tmpfs","Temporários em tmpfs/RAM")] },
    Modulo { numero: 7, nome: "Gamer / Kernel", descricao: "Modo gamer de kernel e priorização de tempo real",
        parametros: &[
            ("gaming.kernel_boost","Boost de kernel em jogos"),("gaming.replay_buffer_ram","Buffer de replay na RAM"),
            ("gaming.fullscreen_silence","Silêncio total em tela cheia"),("gaming.hvci_toggle_per_boot","HVCI toggle por boot"),
            ("system.startup_sandbox","Sandbox de inicialização"),("system.background_freeze","Congelamento de plano de fundo"),
            ("gaming.shader_cache_manager","Gerenciador de cache de shaders"),("gaming.overlay_minimal","Overlay mínimo (Super+G)")] },
    Modulo { numero: 8, nome: "Rede / Segurança", descricao: "Pilha de rede de baixa latência, DNS e segurança",
        parametros: &[
            ("net.dns_encrypted","DNS criptografado (DoH/DoT)"),("net.congestion_control_bbr","Controle de congestionamento BBR"),
            ("net.nic_low_latency","Placa de rede baixa latência"),("net.qos_gaming_voip","QoS para jogos e VoIP"),
            ("net.ipv6_privacy","Privacidade IPv6"),("net.zero_config_share","Compartilhamento zero-config"),
            ("net.ntp_high_precision","NTP de alta precisão"),("net.wifi_band_steering","Wi-Fi band steering"),
            ("net.no_p2p_updates","Sem atualizações P2P"),("net.wireguard_native","WireGuard nativo"),
            ("net.dev_firewall_profiles","Perfis de firewall dev")] },
    Modulo { numero: 9, nome: "Privacidade", descricao: "Privacidade absoluta e soberania do usuário",
        parametros: &[
            ("privacy.zero_telemetry","Telemetria zero"),("privacy.no_ad_id","Sem ID de publicidade"),
            ("privacy.offline_search_only","Busca apenas offline"),("privacy.smart_indexer","Indexador inteligente leve"),
            ("privacy.hardware_kill_switches","Kill switches de hardware"),("privacy.clipboard_guard","Guarda de clipboard"),
            ("privacy.no_nag_screens","Sem telas de insistência"),("privacy.no_silent_installs","Sem instalações silenciosas"),
            ("privacy.clean_uninstall","Desinstalação limpa")] },
    Modulo { numero: 10, nome: "Atualizações / Imutabilidade", descricao: "Atualizações sob controle e imutabilidade",
        parametros: &[
            ("update.manual_or_scheduled","Manual ou agendado"),("update.driver_pinning","Pinning de drivers"),
            ("update.channel_selection","Seleção de canal"),("system.default_apps_lock","Trava de apps padrão"),
            ("system.atomic_rollback","Rollback atômico"),("system.dotfiles_sync","Sincronização de dotfiles"),
            ("system.master_palette","Paleta de comandos mestre")] },
];

fn mostrar_modulos() {
    println!("================================================================");
    println!("  Hyb-Settings — Módulos de Configuração 6 a 10 (Hybridos OS)");
    println!("================================================================");
    println!("Valores lidos de /etc/hybridos/system.toml");
    let doc = match load_doc() { Some(d) => d, None => { println!("[hyb-settings] não foi possível carregar /etc/hybridos/system.toml"); return; } };
    for m in MODULOS_6_10.iter() {
        println!("\n[ Módulo {} — {} ]\n    {}", m.numero, m.nome, m.descricao);
        for (chave, _rot) in m.parametros.iter() {
            let Some((sec, k)) = split_key(chave) else { println!("    {:<45} = <inválido>", chave); continue; };
            let v = doc.get(sec, k).map_or_else(|| String::from("<não definido>"), |x| format_value(x));
            println!("    {:<45} = {}", chave, v);
        }
    }
    println!("\nComandos: hyb-settings list | get <chave> | set <chave> <valor> | help");
    println!("Argumentos lidos de etc/hybridos/hybctl.args (uma linha).");
}

fn cmd_list() {
    let doc = match load_doc() { Some(d) => d, None => { println!("[hyb-settings] não foi possível carregar /etc/hybridos/system.toml"); return; } };
    println!("=== Hybridos OS — Todos os parâmetros (módulos 1-10) ===");
    for s in &doc.sections { println!("\n[{}]", s); for e in doc.entries.iter() { if e.section == *s { println!("  {} = {}", e.key, format_value(&e.value)); } } }
    println!("\nTotal de parâmetros: {}", doc.entries.len());
}
fn cmd_get(key: &str) {
    let doc = match load_doc() { Some(d) => d, None => { println!("[hyb-settings] não foi possível carregar /etc/hybridos/system.toml"); return; } };
    let Some((sec, k)) = split_key(key) else { println!("[hyb-settings] formato inválido (use secao.chave): {}", key); return; };
    match doc.get(sec, k) { Some(v) => println!("{} = {}", key, format_value(v)), None => println!("[hyb-settings] parâmetro não encontrado: {}", key) }
}
fn cmd_set(key: &str, value_raw: &str) {
    let Some((sec, k)) = split_key(key) else { println!("[hyb-settings] formato inválido (use secao.chave): {}", key); return; };
    let val = parse_value(value_raw);
    let mut doc = match load_doc() { Some(d) => d, None => TomlDoc::new() };
    doc.set(sec, k, val);
    let mut out = String::from("# /etc/hybridos/system.toml — Configuração Base do Hybridos OS\n# Atualizado via `hyb-settings`.\n\n");
    for s in &doc.sections { out.push_str(&format!("[{}]\n", s)); for e in doc.entries.iter() { if e.section == *s { out.push_str(&format!("{} = {}\n", e.key, format_value(&e.value))); } } out.push('\n'); }
    if write_file(CONFIG_PATHS[0], &out) { println!("{} = {} (gravado)", key, format_value(&val)); }
    else { println!("[hyb-settings] falha ao gravar {}", CONFIG_PATHS[0]); }
}
fn cmd_help() {
    println!("hyb-settings — Central Unificada de Configurações (módulos 6-10)");
    println!("Sem argumento: exibe módulos 6 a 10 com valores atuais.");
    println!("Comandos: list | get <secao.chave> | set <secao.chave> <v> | help");
    println!("Exemplos: hyb-settings get privacy.zero_telemetry   hyb-settings set gaming.kernel_boost true");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("[hyb-settings] Central Unificada de Configurações do Hybridos OS");
    let args = read_args_file();
    match args {
        Some(ref a) if a.is_empty() => {}
        Some(a) => match a[0].as_str() {
            "list" => { cmd_list(); sys_exit(0); }
            "get" => { if a.len() < 2 { println!("[hyb-settings] uso: hyb-settings get <secao.chave>"); sys_exit(0); } cmd_get(&a[1]); sys_exit(0); }
            "set" => { if a.len() < 3 { println!("[hyb-settings] uso: hyb-settings set <secao.chave> <valor>"); sys_exit(0); } cmd_set(&a[1], &a[2]); sys_exit(0); }
            "help" | "--help" | "-h" => { cmd_help(); sys_exit(0); }
            _ => {}
        },
        None => {}
    }
    mostrar_modulos();
    sys_exit(0);
}

