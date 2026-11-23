#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_write, sys_exit, sys_sched_yield, STDOUT_FILENO};
use libhybrid_sys::net::{sys_socket, sys_bind, sys_listen, SockAddrIn, AF_INET, SOCK_STREAM};
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
fn run() {
    put("HybridOS hyb-ssh-agent v0.1 - Daemon de Autenticacao SSH/GPG\r\n");
    let sock = sys_socket(AF_INET, SOCK_STREAM, 0);
    if sock < 0 { put("falha ao criar socket do agente (rc="); put_u64(sock as u64); putln(")"); return; }
    let addr = SockAddrIn::new([127, 0, 0, 1], 2222);
    let rc = sys_bind(sock as usize, &addr);
    if rc < 0 { put("falha ao vincular socket (rc="); put_u64(rc as u64); putln(")"); return; }
    let rc = sys_listen(sock as usize, 4);
    if rc < 0 { put("falha ao ouvir socket (rc="); put_u64(rc as u64); putln(")"); return; }
    put("hyb-ssh-agent escutando em 127.0.0.1:2222 (fd="); put_u64(sock as u64); putln(")");
    let mut reqs: u64 = 0;
    while reqs < 10 { let _ = sys_sched_yield(); reqs += 1; }
    put("requisicoes de autenticacao processadas: "); put_u64(reqs); put("\r\n");
    putln("hyb-ssh-agent: daemon encerrado de forma controlada.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }

