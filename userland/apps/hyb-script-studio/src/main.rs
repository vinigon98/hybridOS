#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, STDIN_FILENO, STDOUT_FILENO};

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

static BANNER: &str = "\r\nHybridOS hyb-script-studio v0.1 - Debugger de Scripts do Sistema\r\nLer um script da entrada padrao e reportar analise lexica por linha.\r\n";
static PROMPT: &str = "studio> ";

fn read_line(buf: &mut [u8]) -> usize {
    let r = sys_read(STDIN_FILENO, buf);
    if r < 0 { 0 } else { r as usize }
}

fn trimmed_len(buf: &[u8], n: usize) -> usize {
    let mut len = n;
    while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') { len -= 1; }
    len
}

fn analyze(line: &str, line_no: usize) {
    let keywords = ["set", "env", "exec", "if", "then", "fi", "loop", "break", "exit"];
    let mut found = false;
    for kw in keywords.iter() {
        if line.contains(kw) {
            if !found { put("L"); put_u64(line_no as u64); put(": "); found = true; }
            put(kw); put(" ");
        }
    }
    if found { put("| "); putln(line); }
}

fn run() {
    put(BANNER);
    let mut buf = [0u8; 512];
    let mut line_buf = [0u8; 512];
    let mut line_no: usize = 0;
    loop {
        put(PROMPT);
        let n = read_line(&mut buf);
        if n == 0 { putln("(fim do script)"); break; }
        line_no += 1;
        let len = trimmed_len(&buf, n);
        if len == 0 { continue; }
        let mut ci = 0;
        while ci < len { line_buf[ci] = buf[ci]; ci += 1; }
        let line = core::str::from_utf8(&line_buf[..len]).unwrap_or("");
        if line.is_empty() { continue; }
        if line == "exit" || line == "quit" { putln("debugger: sessao encerrada."); return; }
        analyze(line, line_no);
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
