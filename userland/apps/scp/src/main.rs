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
    while n > 0 {
        tmp[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    while pos > 0 {
        pos -= 1;
        put(core::str::from_utf8(&[tmp[pos]]).unwrap_or(""));
    }
}

// O_RDONLY
const O_RDONLY: u32 = 0;
// O_WRONLY | O_CREAT | O_TRUNC
const O_WRONLY_CREAT_TRUNC: u32 = 0x01 | 0x40 | 0x200;
const MODE: u32 = 0o644;

fn run() {
    put("HybridOS scp/sftp v0.1 - Transferencia Segura de Arquivos\r\n");
    let src: &[u8] = b"/sdcard/scp_origem.dat";
    let dst: &[u8] = b"/sdcard/scp_destino.dat";

    let sfd = sys_open(core::str::from_utf8(src).unwrap_or("/sdcard/scp_origem.dat"), O_RDONLY, MODE);
    if sfd < 0 {
        put("nao foi possivel abrir origem (rc=");
        put_u64(sfd as u64);
        putln("); usando demonstracao de memoria");
        // Demo: copia um bloco de memoria para o destino
        let data: &[u8] = b"conteudo protegido via scp/hyb-sftp\n";
        let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/scp_destino.dat"), O_WRONLY_CREAT_TRUNC, MODE);
        if dfd >= 0 {
            let _ = sys_write(dfd as usize, data);
            let _ = sys_close(dfd as usize);
            put("gravou ");
            put_u64(data.len() as u64);
            putln(" bytes no destino (demo)");
        }
        return;
    }

    let mut total: u64 = 0;
    let mut buf = [0u8; 4096];
    let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/scp_destino.dat"), O_WRONLY_CREAT_TRUNC, MODE);
    if dfd < 0 {
        put("nao foi possivel abrir destino (rc=");
        put_u64(dfd as u64);
        putln(")");
        let _ = sys_close(sfd as usize);
        return;
    }
    loop {
        let n = sys_read(sfd as usize, &mut buf);
        if n <= 0 { break; }
        let _ = sys_write(dfd as usize, &buf[..n as usize]);
        total += n as u64;
    }
    let _ = sys_close(dfd as usize);
    let _ = sys_close(sfd as usize);
    put("scp: copiou ");
    put_u64(total);
    putln(" bytes de origem para destino.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
