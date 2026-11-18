#![no_std]
#![no_main]

use libhybrid_sys::linux::{
    sys_read, sys_write, sys_exit, sys_getpid, sys_sysinfo, STDIN_FILENO, STDOUT_FILENO,
};
use libhybrid_sys::linux::SysInfo;

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

static BANNER: &str = "\r\nHybridOS hyb-sh v0.1 - Shell Hibrido Estruturado\r\nBuiltins: help, pid, sysinfo, echo, exit\r\nSuporta pipelines simplificados (segmentos separados por '|').\r\n";
static PROMPT: &str = "hyb-sh> ";
static HELP: &str = "help   - esta ajuda\npid    - mostra o PID do shell\nsysinfo - snapshot de memoria e escalonador do kernel\necho <texto> - ecoa o argumento\nexit   - encerra o shell\n";

fn read_line(buf: &mut [u8]) -> usize {
    let r = sys_read(STDIN_FILENO, buf);
    if r < 0 { 0 } else { r as usize }
}

fn trimmed_len(buf: &[u8], n: usize) -> usize {
    let mut len = n;
    while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') {
        len -= 1;
    }
    len
}

// Divide a entrada em segmentos de pipeline separados por '|'.
fn split_pipeline<'a>(line: &'a str, out: &mut [&'a str]) -> usize {
    let mut count = 0;
    for seg in line.split('|') {
        if count < out.len() {
            out[count] = seg;
            count += 1;
        }
    }
    count
}

fn run() {
    put(BANNER);
    let mut buf = [0u8; 512];
    let mut line_buf = [0u8; 512];
    loop {
        put(PROMPT);
        let n = read_line(&mut buf);
        if n == 0 {
            putln("(sem entrada)");
            continue;
        }
        // Copia a linha para um buffer proprio, liberando `buf` para a proxima leitura.
        let len = trimmed_len(&buf, n);
        if len == 0 {
            continue;
        }
        let mut ci = 0;
        while ci < len {
            line_buf[ci] = buf[ci];
            ci += 1;
        }
        let line = core::str::from_utf8(&line_buf[..len]).unwrap_or("");
        if line.is_empty() {
            continue;
        }
        let mut segs: [&str; 8] = [""; 8];
        let np = split_pipeline(line, &mut segs);
        for i in 0..np {
            let seg = segs[i].trim();
            if seg.starts_with("echo ") {
                put("echo> ");
                putln(&seg[5..]);
            } else if seg == "pid" {
                let pid = sys_getpid();
                put("pid=");
                put_u64(pid as u64);
                put("\r\n");
            } else if seg == "sysinfo" {
                let mut info = SysInfo::default();
                let rc = sys_sysinfo(&mut info);
                put("sysinfo rc=");
                put_u64(rc as u64);
                put(" cpu=");
                put_u64(info.cpu_count as u64);
                put(" mem_total=");
                put_u64(info.total_mem_bytes);
                put(" mem_free=");
                put_u64(info.free_mem_bytes);
                put(" ctx_switches=");
                put_u64(info.context_switches);
                put("\r\n");
            } else if seg == "help" {
                put(HELP);
            } else if seg == "exit" {
                putln("encerrando hyb-sh...");
                return;
            } else {
                put("sh: comando nao reconhecido: ");
                putln(seg);
            }
        }
        let _ = sys_write(STDOUT_FILENO, b"\r\n");
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
