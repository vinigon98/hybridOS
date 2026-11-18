#![no_std] #![no_main]
use libhybrid_sys::linux::{sys_write, sys_exit, STDOUT_FILENO};
use libhybrid_sys::net::{sys_socket, sys_connect, sys_sendto, sys_recvfrom, SockAddrIn, AF_INET, SOCK_STREAM};
fn put(s: &str) { let _ = sys_write(STDOUT_FILENO, s.as_bytes()); }
fn putln(s: &str) { put(s); put("\r\n"); }
fn put_u64(v: u64) { if v == 0 { put("0"); return; } let mut tmp = [0u8; 20]; let mut n = v; let mut pos = 0; while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); } }
fn run() {
    put("HybridOS ssh v0.1 - Cliente OpenSSH Nativo\r\n");
    put("conectando a 127.0.0.1:22 ...\r\n");
    let sock = sys_socket(AF_INET, SOCK_STREAM, 0);
    if sock < 0 { put("falha ao criar socket (rc="); put_u64(sock as u64); putln(")"); return; }
    put("socket criado: fd="); put_u64(sock as u64); put("\r\n");
    let addr = SockAddrIn::new([127, 0, 0, 1], 22);
    let rc = sys_connect(sock as usize, &addr);
    if rc < 0 { put("falha ao conectar (rc="); put_u64(rc as u64); putln(")"); return; }
    putln("conectado ao servidor SSH.");
    let banner: &[u8] = b"SSH-2.0-HybridOS_0.1\r\n";
    let sent = sys_sendto(sock as usize, banner, 0, core::ptr::null());
    if sent < 0 { put("falha ao enviar banner (rc="); put_u64(sent as u64); putln(")"); return; }
    put("banner SSH enviado: "); put_u64(sent as u64); put(" bytes\r\n");
    let mut buf = [0u8; 256];
    let rc = sys_recvfrom(sock as usize, &mut buf, 0, core::ptr::null_mut(), core::ptr::null_mut());
    if rc < 0 { put("falha ao receber resposta (rc="); put_u64(rc as u64); putln(")"); }
    else { putln("resposta do servidor:"); let _ = sys_write(STDOUT_FILENO, &buf[..rc as usize]); put("\r\n"); }
    putln("hyb-ssh: sessao SSH encerrada.");
}
#[no_mangle] pub extern "C" fn _start() -> ! { run(); sys_exit(0); }
