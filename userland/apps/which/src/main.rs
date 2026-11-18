#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_open, sys_read, sys_write, sys_close, sys_exit, STDIN_FILENO, STDOUT_FILENO};

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
const MODE: u32 = 0o644;

static DIRS: [&str; 4] = ["/bin", "/usr/bin", "/sbin", "/usr/sbin"];

fn try_open(dir: &str, cmd: &str, out: &mut [u8; 256]) -> isize {
    let dir_b = dir.as_bytes();
    let cmd_b = cmd.as_bytes();
    let mut pos = 0;
    let mut i = 0;
    while i < dir_b.len() && pos < 255 {
        out[pos] = dir_b[i]; pos += 1; i += 1;
    }
    if pos < 255 { out[pos] = b'/'; pos += 1; }
    i = 0;
    while i < cmd_b.len() && pos < 255 {
        out[pos] = cmd_b[i]; pos += 1; i += 1;
    }
    if pos < 255 { out[pos] = 0; }
    let path = core::str::from_utf8(&out[..pos]).unwrap_or("");
    sys_open(path, O_RDONLY, MODE)
}

fn run() {
    put("HybridOS which/where v0.1 - Resolvedor de Binarios\r\n");
    put("digite um comando por linha (vazio para sair):\r\n");

    let mut buf = [0u8; 128];
    let mut path = [0u8; 256];
    loop {
        put("which> ");
        let n = sys_read(STDIN_FILENO, &mut buf);
        if n <= 0 { break; }
        let mut len = n as usize;
        while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') { len -= 1; }
        if len == 0 { break; }
        let cmd = core::str::from_utf8(&buf[..len]).unwrap_or("");
        let mut found = false;
        for d in DIRS.iter() {
            let fd = try_open(d, cmd, &mut path);
            if fd >= 0 {
                put("encontrado: ");
                put(d);
                put("/");
                putln(cmd);
                let _ = sys_close(fd as usize);
                found = true;
                break;
            }
        }
        if !found {
            put("nao encontrado: ");
            putln(cmd);
        }
    }
    putln("which: resolvido.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
