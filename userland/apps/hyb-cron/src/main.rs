#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit, sys_sysinfo, STDOUT_FILENO};
use libhybrid_sys::linux::SysInfo;
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
const O_RDONLY: u32 = 0; const MODE: u32 = 0o644; static CRONTAB: &str = "/etc/crontab";
fn run() {
    put("HybridOS hyb-cron v0.1 - Agendador de Tarefas e Eventos do Sistema\r\n");
    let mut info = SysInfo::default(); let rc = sys_sysinfo(&mut info);
    put("uptime_kernel: "); put_u64(info.uptime_ticks); put(" ticks | contexto: "); put_u64(info.context_switches); put(" | sysinfo_rc="); put_u64(rc as u64); put("\r\n\r\n");
    put("Crontab do sistema: "); put(CRONTAB); put("\r\n");
    let fd = sys_open(CRONTAB, O_RDONLY, MODE);
    if fd < 0 { put("crontab nao disponivel (rc="); put_u64(fd as u64); putln("); usando tabela de demonstracao"); let jobs: &[&str] = &["0 2 * * *   hyb-backup --full","30 4 * * 0  hyb-limpeza --cache","*/15 * * * * hyb-monitor --relatorio"]; for j in jobs.iter() { put("  [agendado] "); putln(j); } }
    else { let mut buf = [0u8; 4096]; let n = sys_read(fd as usize, &mut buf); let _ = sys_close(fd as usize); if n > 0 { let txt = core::str::from_utf8(&buf[..n as usize]).unwrap_or(""); for line in txt.lines() { if line.is_empty() || line.starts_with('#') { continue; } put("  [cron] "); putln(line); } } }
    putln("\nhyb-cron: agendador listado.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }

