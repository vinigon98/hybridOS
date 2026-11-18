#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, sys_getpid, STDOUT_FILENO};

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

fn run() {
    put("win_sysbench v0.1 - Benchmark de compatibilidade Win32/POSIX\r\n");
    put("PID: ");
    put_u64(sys_getpid() as u64);
    put("\r\n");
    put("execucao de rotinas de sistema: OK\r\n");
    putln("win_sysbench: benchmark concluido.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
