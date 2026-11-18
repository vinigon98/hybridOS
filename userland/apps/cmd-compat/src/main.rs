#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_getpid, sys_write, sys_exit, STDOUT_FILENO};

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

// Tabela de compatibilidade Win32 <-> comando nativo HybridOS/POSIX.
struct CmdMap {
    win: &'static str,
    posix: &'static str,
    desc: &'static str,
}

static MAP: [CmdMap; 8] = [
    CmdMap { win: "DIR", posix: "ls", desc: "lista o conteudo de um diretorio" },
    CmdMap { win: "CLS", posix: "clear", desc: "limpa a tela do terminal" },
    CmdMap { win: "COPY", posix: "cp", desc: "copia um arquivo" },
    CmdMap { win: "DEL", posix: "rm", desc: "remove um arquivo" },
    CmdMap { win: "MD", posix: "mkdir", desc: "cria um diretorio" },
    CmdMap { win: "RD", posix: "rmdir", desc: "remove um diretorio" },
    CmdMap { win: "TYPE", posix: "cat", desc: "exibe o conteudo de um arquivo" },
    CmdMap { win: "EXIT", posix: "exit", desc: "encerra o interpretador" },
];

fn run() {
    put("HybridOS cmd-compat v0.1 - Interpretador de Compatibilidade Win32/POSIX\r\n");
    put("Personalidade atual: HYBRIDOS (ring 3)\r\n");
    put("PID do interpretador: ");
    put_u64(sys_getpid() as u64);
    put("\r\n\r\n");
    put("Mapeamento de comandos:\r\n");
    for m in MAP.iter() {
        put("  ");
        put(m.win);
        put("  ->  ");
        put(m.posix);
        put("  : ");
        putln(m.desc);
    }
    putln("\nDigita um comando Win32 para ver o equivalente nativo (ex: DIR).");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
