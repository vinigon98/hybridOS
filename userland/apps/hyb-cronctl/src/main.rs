#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, sys_getpid, STDIN_FILENO, STDOUT_FILENO};
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
fn read_line(buf: &mut [u8]) -> usize { let r = sys_read(STDIN_FILENO, buf); if r < 0 { 0 } else { r as usize } }
fn trim(buf: &[u8], n: usize) -> &str { let mut len = n; while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') { len -= 1; } core::str::from_utf8(&buf[..len]).unwrap_or("") }
fn copy_into<'a>(dst: &'a mut [u8], s: &str) -> &'a str { let sb = s.as_bytes(); let mut i = 0; while i < sb.len() && i < dst.len() { dst[i] = sb[i]; i += 1; } let len = i.min(dst.len()); core::str::from_utf8(&dst[..len]).unwrap_or("") }
fn run() {
    put("HybridOS hyb-cronctl v0.1 - CLI do Agendador\r\n"); put("PID: "); put_u64(sys_getpid() as u64); put("\r\n"); put("Comandos: list | add <tarefa> | remove <n> | sair\r\n");
    let mut jobs_buf: [[u8; 64]; 8] = [[0; 64]; 8]; let mut jobs_len: [usize; 8] = [0; 8]; let mut count: usize = 0;
    let mut buf = [0u8; 256];
    loop { put("cronctl> "); let n = read_line(&mut buf); if n == 0 { continue; } let line = trim(&buf, n); if line == "sair" || line == "exit" { break; }
        if line == "list" { put("tarefas agendadas: "); put_u64(count as u64); put("\r\n"); for i in 0..count { let s = core::str::from_utf8(&jobs_buf[i][..jobs_len[i]]).unwrap_or(""); put("  "); put_u64(i as u64); put(": "); putln(s); } }
        else if let Some(rest) = line.strip_prefix("add ") { if count < jobs_buf.len() { let sb = rest.as_bytes(); let mut i = 0; while i < sb.len() && i < 64 { jobs_buf[count][i] = sb[i]; i += 1; } jobs_len[count] = i.min(64); count += 1; putln("tarefa adicionada."); } else { putln("agendador cheio."); } }
        else if let Some(rest) = line.strip_prefix("remove ") { let idx: usize = rest.trim().parse().unwrap_or(99); if idx < count { jobs_len[idx] = 0; putln("tarefa removida."); } else { putln("indice invalido."); } } else { putln("comando desconhecido."); } }
    putln("hyb-cronctl: encerrado.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }
