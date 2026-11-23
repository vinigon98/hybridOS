//! hybctl: Controlador Central de Configuração do Sistema (Hybridos OS).
//! Lê/grava parâmetros de /etc/hybridos/system.toml (100 parâmetros, 10 módulos).
//! Argumentos lidos de etc/hybridos/hybctl.args (uma linha) ou modo de ajuda.

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
    println!("[hybctl] erro interno inesperado");
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
fn open_write(path: &str) -> Option<usize> { let fd = sys_open(path, O_WRONLY | O_CREAT | O_TRUNC, 0o644); if fd < 0 { return None; } Some(fd as usize) }
fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match open_readonly(path) { Some(f) => f, None => return None };
    let mut out = Vec::new(); let mut buf = [0u8; 4096];
    loop { let n = sys_read(fd, &mut buf); if n <= 0 { break; } out.extend_from_slice(&buf[..n as usize]); }
    let _ = sys_close(fd); Some(out)
}
fn write_file(path: &str, contents: &str) -> bool {
    let fd = match open_write(path) { Some(f) => f, None => return false };
    let bytes = contents.as_bytes(); let mut w = 0usize;
    while w < bytes.len() { let n = sys_write(fd, &bytes[w..]); if n <= 0 { let _ = sys_close(fd); return false; } w += n as usize; }
    let _ = sys_close(fd); true
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
fn read_stdin_line() -> Option<String> {
    let mut buf = [0u8; 1024];
    let n = sys_read(0, &mut buf);
    if n <= 0 { return None; }
    core::str::from_utf8(&buf[..n as usize]).ok().map(|s| String::from(s.trim_end_matches(['\n','\r'])))
}

fn cmd_get(key: &str) {
    let doc = match load_doc() { Some(d) => d, None => { println!("[hybctl] não foi possível carregar /etc/hybridos/system.toml"); sys_exit(0); } };
    let Some((sec, k)) = split_key(key) else { println!("[hybctl] formato inválido (use secao.chave): {}", key); sys_exit(0); };
    match doc.get(sec, k) { Some(v) => println!("{} = {}", key, format_value(v)), None => println!("[hybctl] parâmetro não encontrado: {}", key) }
}
fn cmd_set(key: &str, value_raw: &str) {
    let Some((sec, k)) = split_key(key) else { println!("[hybctl] formato inválido (use secao.chave): {}", key); sys_exit(0); };
    let val = parse_value(value_raw);
    let mut doc = match load_doc() { Some(d) => d, None => TomlDoc::new() };
    doc.set(sec, k, val);
    let mut out = String::from("# /etc/hybridos/system.toml — Configuração Base do Hybridos OS\n# Atualizado via `hybctl set`.\n\n");
    for s in &doc.sections { out.push_str(&format!("[{}]\n", s)); for e in doc.entries.iter() { if e.section == *s { out.push_str(&format!("{} = {}\n", e.key, format_value(&e.value))); } } out.push('\n'); }
    let root: Vec<&TomlEntry> = doc.entries.iter().filter(|e| e.section == "__root__").collect();
    if !root.is_empty() { out.push_str("# (sem seção)\n"); for e in &root { out.push_str(&format!("{} = {}\n", e.key, format_value(&e.value))); } }
    if write_file(CONFIG_PATHS[0], &out) { println!("{} = {} (gravado em {})", key, format_value(&val), CONFIG_PATHS[0]); }
    else { println!("[hybctl] falha ao gravar {}", CONFIG_PATHS[0]); }
}
fn cmd_list() {
    let doc = match load_doc() { Some(d) => d, None => { println!("[hybctl] não foi possível carregar /etc/hybridos/system.toml"); sys_exit(0); } };
    println!("=== Hybridos OS — Parâmetros de Configuração ===");
    for s in &doc.sections { println!("\n[{}]", s); for e in doc.entries.iter() { if e.section == *s { println!("  {} = {}", e.key, format_value(&e.value)); } } }
    println!("\nTotal de parâmetros: {}", doc.entries.len());
}
fn cmd_help() {
    println!("hybctl — Controlador Central de Configuração do Hybridos OS");
    println!("Comandos: get <secao.chave> | set <secao.chave> <v> | list | help");
    println!("Módulos: 1 display 2 audio 3 input 4 power 5 shell | 6 developer 7 gaming 8 net 9 privacy 10 update");
    println!("Exemplos: hybctl get gaming.kernel_boost   hybctl get privacy.zero_telemetry   hybctl set net.dns_encrypted true");
    println!("Argumentos lidos de etc/hybridos/hybctl.args (uma linha) ou este help.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("[hybctl] Controlador Central de Configuração do Hybridos OS");
    let args = read_args_file().unwrap_or_else(|| {
        match read_stdin_line() { Some(lin) if !lin.is_empty() => { let mut o = Vec::new(); for t in lin.split_whitespace() { o.push(String::from(t)); } o } _ => Vec::new() }
    });
    if args.is_empty() { cmd_help(); sys_exit(0); }
    match args[0].as_str() {
        "get" => { if args.len() < 2 { println!("[hybctl] uso: hybctl get <secao.chave>"); sys_exit(0); } cmd_get(&args[1]); }
        "set" => { if args.len() < 3 { println!("[hybctl] uso: hybctl set <secao.chave> <valor>"); sys_exit(0); } cmd_set(&args[1], &args[2]); }
        "list" => { cmd_list(); }
        "help" | "--help" | "-h" => { cmd_help(); }
        other => { println!("[hybctl] comando desconhecido: {}", other); println!("[hybctl] digite 'hybctl help'"); }
    }
    sys_exit(0);
}
