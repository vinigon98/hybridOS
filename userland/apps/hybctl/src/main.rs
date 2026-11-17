//! hybctl — Controlador Central de Configuração do Sistema Hybridos OS.
//! Ring 3, #![no_std], #![no_main], alvo x86_64-unknown-none. Agente 6.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::println;
use libhybrid_sys::linux::{sys_close, sys_exit, sys_open, sys_read, sys_write};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hybctl] erro interno inesperado");
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn abort() -> ! { loop { core::hint::spin_loop(); } }

const CONFIG_PATH: &str = "etc/hybridos/system.toml";
const ARGS_PATH: &str = "etc/hybridos/hybctl.args";
const O_RDONLY: u32 = 0; const O_WRONLY: u32 = 1;
const O_CREAT: u32 = 0x40; const O_TRUNC: u32 = 0x200; const MODE_0644: u32 = 0o644;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let args_text = read_args();
    let words: alloc::vec::Vec<&str> = args_text.split_whitespace().collect();
    if words.is_empty() { print_usage(); sys_exit(0); }
    match words[0] {
        "get" => { if words.len()!=2 { eprintln_bare("Uso: hybctl get <parametro>"); sys_exit(2); } cmd_get(words[1]); }
        "set" => { if words.len()!=3 { eprintln_bare("Uso: hybctl set <parametro> <valor>"); sys_exit(2); } cmd_set(words[1], words[2]); }
        "list" => { cmd_list(words.get(1).copied()); }
        "help" => { print_usage(); }
        other => { eprintln_bare(&format!("hybctl: comando desconhecido: {other}")); print_usage(); sys_exit(2); }
    }
    sys_exit(0);
}

fn read_args() -> alloc::string::String {
    match read_file(ARGS_PATH) { Some(b) => match alloc::string::String::from_utf8(b) { Ok(t)=>t, Err(_)=>alloc::string::String::new() }, None=>alloc::string::String::new() }
}
fn open_readonly(path: &str) -> Option<usize> { let fd=sys_open(path,O_RDONLY,0); if fd<0 { return None; } Some(fd as usize) }
fn open_write(path: &str) -> Option<usize> { let fd=sys_open(path,O_WRONLY|O_CREAT|O_TRUNC,MODE_0644); if fd<0 { return None; } Some(fd as usize) }
fn read_file(path: &str) -> Option<alloc::vec::Vec<u8>> {
    let fd=match open_readonly(path){Some(f)=>f,None=>return None};
    let mut out:alloc::vec::Vec<u8>=alloc::vec::Vec::new(); let mut buf=[0u8;4096];
    loop { let n=sys_read(fd,&mut buf); if n<=0 { break; } out.extend_from_slice(&buf[..n as usize]); }
    let _=sys_close(fd); Some(out)
}
fn write_file(path: &str, contents: &str) -> bool {
    let fd=match open_write(path){Some(f)=>f,None=>return false};
    let bytes=contents.as_bytes(); let mut w=0usize;
    while w<bytes.len() { let n=sys_write(fd,&bytes[w..]); if n<=0 { let _=sys_close(fd); return false; } w+=n as usize; }
    let _=sys_close(fd); true
}
#[derive(Debug,Clone,PartialEq)] enum TomlValue { Bool(bool), Integer(i64), Str(alloc::string::String) }
impl TomlValue {
    fn parse(s:&str)->TomlValue { let t=s.trim(); if t.eq_ignore_ascii_case("true"){return TomlValue::Bool(true);} if t.eq_ignore_ascii_case("false"){return TomlValue::Bool(false);} if let Ok(n)=t.parse::<i64>(){return TomlValue::Integer(n);} let st=t.strip_prefix('"').and_then(|i|i.strip_suffix('"')); match st { Some(i)=>TomlValue::Str(unescape_toml_string(i)), None=>TomlValue::Str(alloc::string::String::from(t)) } }
    fn to_toml(&self)->alloc::string::String { match self { TomlValue::Bool(b)=>b.to_string(), TomlValue::Integer(n)=>n.to_string(), TomlValue::Str(s)=>format!("\"{}\"",escape_toml_string(s)) } }
    fn display(&self)->alloc::string::String { match self { TomlValue::Bool(b)=>b.to_string(), TomlValue::Integer(n)=>n.to_string(), TomlValue::Str(s)=>s.clone() } }
}
fn escape_toml_string(s:&str)->alloc::string::String { let mut out=alloc::string::String::with_capacity(s.len()); for c in s.chars() { match c { '\\'=>out.push_str("\\\\"), '"'=>out.push_str("\\\""), '\n'=>out.push_str("\\n"), '\r'=>out.push_str("\\r"), '\t'=>out.push_str("\\t"), c=>out.push(c); } } out }
fn unescape_toml_string(s:&str)->alloc::string::String { let mut out=alloc::string::String::with_capacity(s.len()); let mut cs=s.chars().peekable(); while let Some(c)=cs.next() { if c=='\\' { if let Some(n)=cs.next() { out.push(match n{ 'n'=>'\n','r'=>'\r','t'=>'\t','"'=>'"','\\'=>'\\', c=>c }); } } else { out.push(c); } } out }
struct Section { name:alloc::string::String, entries:alloc::vec::Vec<(alloc::string::String,TomlValue)> }
impl Section { fn new(n:&str)->Section { Section{name:n.to_string(),entries:alloc::vec::Vec::new()} } fn find_entry(&self,k:&str)->Option<usize> { let mut i=0; while i<self.entries.len(){ if self.entries[i].0==k { return Some(i); } i+=1; } None } }
struct Config { sections:alloc::vec::Vec<Section> }
impl Config {
    fn new()->Config { Config{sections:alloc::vec::Vec::new()} }
    fn section_index(&self,n:&str)->Option<usize> { let mut i=0; while i<self.sections.len(){ if self.sections[i].name==n { return Some(i); } i+=1; } None }
    fn get(&self, full:&str)->Option<&TomlValue> { let (s,k)=split_key(full)?; let i=self.section_index(s)?; let e=self.sections[i].find_entry(k)?; Some(&self.sections[i].entries[e].1) }
    fn set(&mut self, full:&str, v:TomlValue)->bool { let (s,k)=match split_key(full){Some(p)=>p,None=>return false}; let i=match self.section_index(s){Some(i)=>i,None=>{self.sections.push(Section::new(s)); self.sections.len()-1}}; match self.sections[i].find_entry(k){ Some(e)=>{ self.sections[i].entries[e].1=v; } None=>{ self.sections[i].entries.push((alloc::string::String::from(k), v)); } } true }
    fn to_toml_string(&self)->alloc::string::String { let mut out=alloc::string::String::new(); out.push_str("# /etc/hybridos/system.toml — Hybridos OS\n# gerido pelo `hybctl` (Agente 6 — Módulos 1 a 5).\n\n"); for sec in &self.sections { out.push_str(&format!("[{}]\n", sec.name)); for (k,v) in &sec.entries { out.push_str(&format!("{} = {}\n", k, v.to_toml())); } out.push('\n'); } out }
}
fn parse_toml(text:&str)->Config { let mut cfg=Config::new(); let mut cur:Option<alloc::string::String>=None; for raw in text.lines() { let line=raw.trim(); if line.is_empty() || line.starts_with('#') { continue; } if let Some(rest)=line.strip_prefix('[').and_then(|i|i.strip_suffix(']')) { let n=rest.trim(); if !n.is_empty() { if cfg.section_index(n).is_none() { cfg.sections.push(Section::new(n)); } cur=Some(alloc::string::String::from(n)); } continue; } if let Some(eq)=line.find('=') { let k=line[..eq].trim(); let mut vt=&line[eq+1..]; vt=strip_inline_comment(vt); if k.is_empty() { continue; } let v=TomlValue::parse(vt); let sn=cur.clone().unwrap_or_else(||alloc::string::String::from("default")); if cfg.section_index(&sn).is_none() { cfg.sections.push(Section::new(&sn)); } let i=cfg.section_index(&sn).unwrap(); match cfg.sections[i].entries.iter().position(|e|e.key==k){ Some(p)=>{ cfg.sections[i].entries[p].1=v; } None=>{ cfg.sections[i].entries.push((alloc::string::String::from(k), v)); } } } } cfg }
fn strip_inline_comment(s:&str)->&str { let mut q=false; for (i,c) in s.char_indices() { match c { '"'=>q=!q, '#' if !q=>return s[..i].trim(), _=>{} } } s }
fn split_key(full:&str)->Option<(&str,&str)> { let p=full.rfind('.')?; if p==0||p==full.len()-1 { return None; } Some((&full[..p], &full[p+1..])) }
fn print_usage() { println!("hybctl — Controlador Central de Configuração do Hybridos OS"); println!("Arquivo: {}", CONFIG_PATH); println!("Argumentos: {}", ARGS_PATH); println!("Uso: hybctl get|set|list|help"); }
fn eprintln_bare(t:&str) { let _=sys_write(2,t.as_bytes()); let _=sys_write(2,b"\n"); }
fn cmd_get(p:&str) { let text=match read_file(CONFIG_PATH){Some(t)=>t,None=>{eprintln_bare(&format!("hybctl: não foi possível ler {}",CONFIG_PATH)); sys_exit(1);}}; let text=match alloc::string::String::from_utf8(text){Ok(t)=>t,Err(_)=>{eprintln_bare(&format!("hybctl: {} não é UTF-8",CONFIG_PATH)); sys_exit(1);}}; let cfg=parse_toml(&text); match cfg.get(p){ Some(v)=>{ let _=sys_write(1,format!("{} = {}\n",p,v.display()).as_bytes()); } None=>{ eprintln_bare(&format!("hybctl: parâmetro não encontrado: {}",p)); sys_exit(1); } } }
fn cmd_set(p:&str, vt:&str) { let text=match read_file(CONFIG_PATH){Some(t)=>t,None=>{eprintln_bare(&format!("hybctl: não foi possível ler {}",CONFIG_PATH)); sys_exit(1);}}; let text=match alloc::string::String::from_utf8(text){Ok(t)=>t,Err(_)=>{eprintln_bare(&format!("hybctl: {} não é UTF-8",CONFIG_PATH)); sys_exit(1);}}; let mut cfg=parse_toml(&text); let v=TomlValue::parse(vt); let disp=v.display(); if !cfg.set(p,v){ eprintln_bare(&format!("hybctl: parâmetro inválido: {}",p)); sys_exit(1); } if !write_file(CONFIG_PATH,&cfg.to_toml_string()){ eprintln_bare(&format!("hybctl: falha ao gravar {}",CONFIG_PATH)); sys_exit(1); } let _=sys_write(1,format!("{} = {}\n",p,disp).as_bytes()); }
fn cmd_list(m:Option<&str>) { let text=match read_file(CONFIG_PATH){Some(t)=>t,None=>{eprintln_bare(&format!("hybctl: não foi possível ler {}",CONFIG_PATH)); sys_exit(1);}}; let text=match alloc::string::String::from_utf8(text){Ok(t)=>t,Err(_)=>alloc::string::String::new()}; let cfg=parse_toml(&text); match m { Some(md)=>match cfg.section_index(md){ Some(i)=>{ let s=&cfg.sections[i]; let _=sys_write(1,format!("Seção [{}]\n",s.name).as_bytes()); for (k,v) in &s.entries { let _=sys_write(1,format!("  {} = {}\n",k,v.display()).as_bytes()); } } None=>{ eprintln_bare(&format!("hybctl: seção não encontrada: {}",md)); sys_exit(1); } } None=>{ let _=sys_write(1,format!("Parâmetros ({} seções):\n",cfg.sections.len()).as_bytes()); for s in &cfg.sections { let _=sys_write(1,format!("  [{}] — {} parâmetros\n",s.name,s.entries.len()).as_bytes()); for (k,v) in &s.entries { let _=sys_write(1,format!("      {}.{} = {}\n",s.name,k,v.display()).as_bytes()); } } } } }
