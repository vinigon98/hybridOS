#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit, STDIN_FILENO, STDOUT_FILENO};
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
const O_RDONLY: u32 = 0; const MODE: u32 = 0o644;
fn run() {
    put("HybridOS hyb-grep v0.1 - Buscador de Código Ultrarrápido\r\n");
    put("digite o padrao de busca (uma linha):\r\n");
    let mut pat = [0u8; 64]; let n = sys_read(STDIN_FILENO, &mut pat);
    if n <= 0 { putln("sem padrao informado."); return; }
    let mut plen = n as usize; while plen > 0 && (pat[plen - 1] == b'\n' || pat[plen - 1] == b'\r') { plen -= 1; } if plen == 0 { putln("padrao vazio."); return; }
    let pattern = core::str::from_utf8(&pat[..plen]).unwrap_or("");
    put("buscando em: /var/log/hyb.log\r\n");
    let fd = sys_open("/var/log/hyb.log", O_RDONLY, MODE);
    if fd < 0 { put("nao foi possivel abrir /var/log/hyb.log (rc="); put_u64(fd as u64); putln("); usando demonstracao em memoria");
        let demo: &[u8] = b"erro: falha de gravacao\ninfo: inicializacao ok\nerro: timeout de rede\ndebug: varrendo blocos\n";
        let mut line_no: u64 = 1; let mut start = 0usize; let mut matched = false;
        for (i, &b) in demo.iter().enumerate() { if b == b'\n' { let line = core::str::from_utf8(&demo[start..i]).unwrap_or(""); if line.contains(pattern) { put_u64(line_no); put(": "); putln(line); matched = true; } start = i + 1; line_no += 1; } }
        if !matched { put("nenhuma coincidencia para: "); putln(pattern); } return; }
    let mut buf = [0u8; 4096]; let mut line_no: u64 = 1; let mut line_start = 0usize; let mut matched = false;
    loop { let n = sys_read(fd as usize, &mut buf); if n <= 0 { break; } let mut i = 0;
        while i < n as usize { if buf[i] == b'\n' { let line = core::str::from_utf8(&buf[line_start..i]).unwrap_or(""); if line.contains(pattern) { put_u64(line_no); put(": "); putln(line); matched = true; } line_start = i + 1; line_no += 1; } i += 1; } }
    let _ = sys_close(fd as usize);
    if !matched { put("nenhuma coincidencia para: "); putln(pattern); } putln("hyb-grep: busca concluida.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }
