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

static BANNER: &str = "\r\nHybridOS hyb-term v0.1 - Emulador de Terminal Acelerado por GPU\r\nComandos: help, clear, exit\r\n";
static PROMPT: &str = "hyb-term> ";
static HELP: &str = "help  - esta ajuda\nclear - limpar a tela\nexit  - encerra o terminal\n";

fn read_line(buf: &mut [u8]) -> usize {
    let r = sys_read(STDIN_FILENO, buf);
    if r < 0 {
        0
    } else {
        r as usize
    }
}

fn run() {
    put(BANNER);
    let mut buf = [0u8; 256];
    loop {
        put(PROMPT);
        let n = read_line(&mut buf);
        if n == 0 {
            putln("(sem entrada)");
            continue;
        }
        let mut len = n;
        while len > 0 && (buf[len - 1] == b'\n' || buf[len - 1] == b'\r') {
            len -= 1;
        }
        if len == 0 {
            continue;
        }
        let line = core::str::from_utf8(&buf[..len]).unwrap_or("");
        match line {
            "help" => put(HELP),
            "clear" => put("\x1b[2J\x1b[H"),
            "exit" => {
                putln("encerrando hyb-term...");
                return;
            }
            _ => {
                put("eco> ");
                put(line);
                put("\r\n");
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
