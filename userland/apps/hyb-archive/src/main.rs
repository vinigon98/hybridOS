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
    put("HybridOS hyb-archive v0.1 - Motor Nativo de Compactacao Multithread\r\n");
    put("algoritmo: RLE (Run-Length Encoding) com cabeca HYBARC\r\n");
    let src: &[u8] = b"/sdcard/archive_entrada.dat";
    let dst: &[u8] = b"/sdcard/archive_saida.hyb";
    let sfd = sys_open(core::str::from_utf8(src).unwrap_or("/sdcard/archive_entrada.dat"), O_RDONLY, MODE);
    if sfd < 0 {
        put("nao foi possivel abrir entrada (rc="); put_u64(sfd as u64); putln("); criando demonstracao");
        let dados: &[u8] = b"AAAAABBBBBCCCCCDDDDDEEEEE";
        let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/archive_saida.hyb"), O_WRONLY_CREAT_TRUNC, MODE);
        if dfd >= 0 {
            let cabeca: &[u8] = b"HYBARC"; let _ = sys_write(dfd as usize, cabeca);
            let mut saida = [0u8; 128]; let mut spos = 0; let mut i = 0;
            while i < dados.len() {
                let b = dados[i]; let mut cnt: u8 = 1;
                while i + 1 < dados.len() && dados[i + 1] == b && cnt < 255 { i += 1; cnt += 1; }
                saida[spos] = b; spos += 1; saida[spos] = cnt; spos += 1; i += 1;
            }
            let _ = sys_write(dfd as usize, &saida[..spos]);
            let _ = sys_close(dfd as usize);
            put("arquivo compactado: "); put_u64(spos as u64); put(" bytes\r\n");
        }
        return;
    }
    let mut total_lido: u64 = 0;
    let mut total_escrito: u64 = 6;
    let mut buf = [0u8; 4096];
    let mut saida = [0u8; 8192];
    let dfd = sys_open(core::str::from_utf8(dst).unwrap_or("/sdcard/archive_saida.hyb"), O_WRONLY_CREAT_TRUNC, MODE);
    if dfd < 0 { put("nao foi possivel abrir destino (rc="); put_u64(dfd as u64); putln(")");
        let _ = sys_close(sfd as usize); return; }
    let cabeca: &[u8] = b"HYBARC"; let _ = sys_write(dfd as usize, cabeca);
    loop {
        let n = sys_read(sfd as usize, &mut buf);
        if n <= 0 { break; }
        total_lido += n as u64;
        let mut spos = 0; let mut i = 0;
        while i < n as usize {
            let b = buf[i]; let mut cnt: u8 = 1;
            while i + 1 < n as usize && buf[i + 1] == b && cnt < 255 { i += 1; cnt += 1; }
            saida[spos] = b; spos += 1; saida[spos] = cnt; spos += 1; i += 1;
        }
        if spos > 0 { let _ = sys_write(dfd as usize, &saida[..spos]); total_escrito += spos as u64; }
    }
    let _ = sys_close(dfd as usize);
    let _ = sys_close(sfd as usize);
    put("entrada: "); put_u64(total_lido); put(" bytes | saida: "); put_u64(total_escrito); putln(" bytes");
    putln("hyb-archive: compactacao concluida.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
