#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, sys_open, sys_close, STDIN_FILENO, STDOUT_FILENO};
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
const O_WRONLY_CREAT_TRUNC: u32 = 0x01 | 0x40 | 0x200; const MODE: u32 = 0o644;
fn run() {
    put("HybridOS hyb-pipe v0.1 - Utilitario de Redirecionamento para Clipboard\r\n");
    put("redirecionando a entrada padrao para o clipboard (arquivo de transferencia)...\r\n");
    let clipboard: &[u8] = b"/sdcard/clipboard"; let dfd = sys_open(core::str::from_utf8(clipboard).unwrap_or("/sdcard/clipboard"), O_WRONLY_CREAT_TRUNC, MODE);
    if dfd < 0 { put("nao foi possivel abrir clipboard (rc="); put_u64(dfd as u64); putln("); escrevendo na saida padrao"); let mut buf = [0u8; 4096]; let mut total: u64 = 0; loop { let n = sys_read(STDIN_FILENO, &mut buf); if n <= 0 { break; } let _ = sys_write(STDOUT_FILENO, &buf[..n as usize]); total += n as u64; } put("total: "); put_u64(total); putln(" bytes"); return; }
    let mut total: u64 = 0; let mut buf = [0u8; 4096]; loop { let n = sys_read(STDIN_FILENO, &mut buf); if n <= 0 { break; } let _ = sys_write(dfd as usize, &buf[..n as usize]); total += n as u64; }
    let _ = sys_close(dfd as usize); put("clipboard gravado: "); put_u64(total); putln(" bytes"); putln("hyb-pipe: redirecionamento concluido.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }
