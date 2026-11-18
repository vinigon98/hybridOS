#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit, sys_getpid, STDIN_FILENO, STDOUT_FILENO};

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

const O_RDONLY: u32 = 0;
const O_WRONLY_CREAT_TRUNC: u32 = 0x01 | 0x40 | 0x200;
const MODE: u32 = 0o644;
static CONF: &str = "/etc/hybctl.conf";

fn read_line(buf: &mut [u8]) -> usize {
    let r = sys_read(STDIN_FILENO, buf);
    if r < 0 { 0 } else { r as usize }
}

fn trim(buf: &[u8], n: usize) -> &str {
    let mut len = n;
    while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') {
        len -= 1;
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

// Armazena configuracoes como pares chave=valor (até 16 entradas).
#[derive(Copy, Clone)]
struct Entry { key: [u8; 32], klen: usize, val: [u8; 64], vlen: usize, used: bool }

fn run() {
    put("HybridOS hybctl v0.1 - Controlador Central de Configuracao do Sistema\r\n");
    put("PID: ");
    put_u64(sys_getpid() as u64);
    put("\r\n");
    put("Arquivo de configuracao: ");
    put(CONF);
    put("\r\n");
    put("Comandos: list | get <chave> | set <chave> <valor> | sair\r\n");

    let mut entries: [Entry; 16] = [Entry { key: [0;32], klen: 0, val: [0;64], vlen: 0, used: false }; 16];

    // Carrega configuracao existente, se houver.
    let fd = sys_open(CONF, O_RDONLY, MODE);
    if fd >= 0 {
        let mut buf = [0u8; 1024];
        let n = sys_read(fd as usize, &mut buf);
        let _ = sys_close(fd as usize);
        if n > 0 {
            let txt = core::str::from_utf8(&buf[..n as usize]).unwrap_or("");
            let mut slot = 0;
            for line in txt.lines() {
                if slot >= 16 { break; }
                if let Some(pos) = line.find('=') {
                    let (k, v) = line.split_at(pos);
                    let v = &v[1..];
                    let kb = k.as_bytes();
                    let vb = v.as_bytes();
                    let mut i = 0;
                    while i < kb.len() && i < 32 { entries[slot].key[i] = kb[i]; i += 1; }
                    entries[slot].klen = i.min(32);
                    i = 0;
                    while i < vb.len() && i < 64 { entries[slot].val[i] = vb[i]; i += 1; }
                    entries[slot].vlen = i.min(64);
                    entries[slot].used = true;
                    slot += 1;
                }
            }
        }
    }

    let mut buf = [0u8; 256];
    loop {
        put("hybctl> ");
        let n = read_line(&mut buf);
        if n == 0 { continue; }
        let line = trim(&buf, n);
        if line == "sair" || line == "exit" { break; }
        if line == "list" {
            put("chaves configuradas:\r\n");
            for e in entries.iter() {
                if e.used {
                    let _ = sys_write(STDOUT_FILENO, &e.key[..e.klen]);
                    put("=");
                    let _ = sys_write(STDOUT_FILENO, &e.val[..e.vlen]);
                    put("\r\n");
                }
            }
        } else if let Some(rest) = line.strip_prefix("get ") {
            let key = rest.trim();
            let mut found = false;
            for e in entries.iter() {
                if e.used && e.klen == key.len() && &e.key[..e.klen] == key.as_bytes() {
                    put("valor=");
                    let _ = sys_write(STDOUT_FILENO, &e.val[..e.vlen]);
                    put("\r\n");
                    found = true;
                    break;
                }
            }
            if !found { put("chave nao encontrada: "); putln(key); }
        } else if let Some(rest) = line.strip_prefix("set ") {
            if let Some((key, val)) = rest.split_once(' ') {
                let mut placed = false;
                for e in entries.iter_mut() {
                    if !e.used {
                        let kb = key.as_bytes();
                        let vb = val.as_bytes();
                        let mut i = 0;
                        while i < kb.len() && i < 32 { e.key[i] = kb[i]; i += 1; }
                        e.klen = i.min(32);
                        i = 0;
                        while i < vb.len() && i < 64 { e.val[i] = vb[i]; i += 1; }
                        e.vlen = i.min(64);
                        e.used = true;
                        placed = true;
                        break;
                    }
                }
                if placed {
                    // persiste no arquivo (recria inteiro).
                    let dfd = sys_open(CONF, O_WRONLY_CREAT_TRUNC, MODE);
                    if dfd >= 0 {
                        let mut out = [0u8; 1024];
                        let mut opos = 0;
                        for e in entries.iter() {
                            if e.used {
                                let mut i = 0;
                                while i < e.klen && opos < 1023 { out[opos] = e.key[i]; opos += 1; i += 1; }
                                if opos < 1023 { out[opos] = b'='; opos += 1; }
                                i = 0;
                                while i < e.vlen && opos < 1023 { out[opos] = e.val[i]; opos += 1; i += 1; }
                                if opos < 1023 { out[opos] = b'\n'; opos += 1; }
                            }
                        }
                        let _ = sys_write(dfd as usize, &out[..opos]);
                        let _ = sys_close(dfd as usize);
                        putln("configuracao definida e persistida.");
                    } else {
                        putln("definido (nao foi possivel persistir no arquivo).");
                    }
                } else {
                    putln("nao ha slots disponiveis.");
                }
            } else {
                putln("uso: set <chave> <valor>");
            }
        } else {
            putln("comando desconhecido.");
        }
    }
    putln("hybctl: encerrado.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
