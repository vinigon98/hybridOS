#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit, STDOUT_FILENO};

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

const O_RDONLY: u32 = 0;
const O_WRONLY_CREAT_TRUNC: u32 = 0x01 | 0x40 | 0x200;
const MODE: u32 = 0o644;

fn run() {
    put("HybridOS hyb-sync v0.1 - Motor de Copia e Sincronizacao Paralela\r\n");
    let src: &[u8] = b"/sdcard/sync_origem.dat";
    let dst: &[u8] = b"/sdcard/sync_destino.dat";
    let sfd = sys_open(core::str::from_utf8(src).unwrap_or("/sdcard/sync_origem.dat"), O_RDONLY, MODE);
    if sfd < 0 {
        let dados: &[u8] = b"bloco de dados para sincronizacao paralela de arquivo hyb-sync\n";
        let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/sync_destino.dat"), O_WRONLY_CREAT_TRUNC, MODE);
        if dfd >= 0 { let _ = sys_write(dfd as usize, dados); let _ = sys_close(dfd as usize);
            put("sincronizou "); put_u64(dados.len() as u64); putln(" bytes (demo sem arquivo de origem)"); }
        return;
    }
    let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/sync_destino.dat"), O_WRONLY_CREAT_TRUNC, MODE);
    if dfd < 0 { put("nao foi possivel abrir destino (rc="); put_u64(dfd as u64); putln(")");
        let _ = sys_close(sfd as usize); return; }
    let mut total: u64 = 0;
    let mut buf = [0u8; 4096];
    loop {
        let n = sys_read(sfd as usize, &mut buf);
        if n <= 0 { break; }
        let _ = sys_write(dfd as usize, &buf[..n as usize]);
        total += n as u64;
    }
    let _ = sys_close(dfd as usize);
    let _ = sys_close(sfd as usize);
    put("sincronizado: "); put_u64(total); putln(" bytes copiados de origem para destino.");
    putln("hyb-sync: copia e sincronizacao concluídas.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
