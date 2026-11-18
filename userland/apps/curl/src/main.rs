#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, STDOUT_FILENO};
use libhybrid_sys::net::{sys_socket, sys_connect, sys_sendto, sys_recvfrom, SockAddrIn, AF_INET, SOCK_STREAM};

fn put(s: &str) {
    let _ = sys_write(STDOUT_FILENO, s.as_bytes());
}
fn putln(s: &str) {
    put(s);
    put("\r\n");
}
fn put_u64(v: u64) {
    if v == 0 { put("0"); return; }
    let mut tmp = [0u8; 20];
    let mut n = v;
    let mut pos = 0;
    while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); }
}

fn run() {
    put("HybridOS curl/hyb-http v0.1 - Cliente de Requisicoes de Rede\r\n");
    put("requisicao GET a http://127.0.0.1:80/\r\n");
    let sock = sys_socket(AF_INET, SOCK_STREAM, 0);
    if sock < 0 { put("falha ao criar socket (rc="); put_u64(sock as u64); putln(")"); return; }
    let addr = SockAddrIn::new([127, 0, 0, 1], 80);
    let rc = sys_connect(sock as usize, &addr);
    if rc < 0 { put("falha ao conectar (rc="); put_u64(rc as u64); putln(")"); return; }
    putln("conectado ao servidor HTTP.");
    let request: &[u8] = b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let sent = sys_sendto(sock as usize, request, 0, core::ptr::null());
    if sent < 0 { put("falha ao enviar requisicao (rc="); put_u64(sent as u64); putln(")"); return; }
    put("requisicao enviada: "); put_u64(sent as u64); put(" bytes\r\n");
    let mut buf = [0u8; 512];
    let rc = sys_recvfrom(sock as usize, &mut buf, 0, core::ptr::null_mut(), core::ptr::null_mut());
    if rc < 0 { put("falha ao receber resposta (rc="); put_u64(rc as u64); putln(")"); }
    else { putln("resposta HTTP recebida:"); let _ = sys_write(STDOUT_FILENO, &buf[..rc as usize]); put("\r\n"); }
    putln("curl: transferencia concluida.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
