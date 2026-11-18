#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, STDOUT_FILENO};

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

fn num_digits(v: u64) -> usize {
    if v == 0 { return 1; }
    let mut n = v;
    let mut d = 0;
    while n > 0 { n /= 10; d += 1; }
    d
}

struct Package {
    name: &'static str,
    version: &'static str,
    size_kb: u64,
    category: &'static str,
}

static INDEX: [Package; 8] = [
    Package { name: "hyb-term", version: "0.1.0", size_kb: 24, category: "terminais" },
    Package { name: "hyb-sh", version: "0.1.0", size_kb: 32, category: "shells" },
    Package { name: "hyb-pkg", version: "0.1.0", size_kb: 28, category: "gerenciadores" },
    Package { name: "curl", version: "1.0.0", size_kb: 48, category: "rede" },
    Package { name: "ssh", version: "1.0.0", size_kb: 64, category: "rede" },
    Package { name: "hyb-hash", version: "0.1.0", size_kb: 20, category: "criptografia" },
    Package { name: "hyb-grep", version: "0.1.0", size_kb: 18, category: "busca" },
    Package { name: "hyb-archive", version: "0.1.0", size_kb: 22, category: "compactacao" },
];

fn run() {
    put("HybridOS hyb-pkg v0.1 - Gerenciador de Pacotes Universal do OS\r\n");
    put("Indice de pacotes disponiveis:\r\n");
    put("NOME              VER    TAM(KB)  CATEGORIA\r\n");
    for p in INDEX.iter() {
        put("  ");
        put(p.name);
        let pad = 18usize.saturating_sub(p.name.len());
        for _ in 0..pad { put(" "); }
        put(p.version);
        let pad = 6usize.saturating_sub(p.version.len());
        for _ in 0..pad { put(" "); }
        put_u64(p.size_kb);
        let pad = 10usize.saturating_sub(num_digits(p.size_kb));
        for _ in 0..pad { put(" "); }
        putln(p.category);
    }
    put("\nTotal: ");
    put_u64(INDEX.len() as u64);
    putln(" pacotes indexados.");
    putln("Uso: hyb-pkg list | hyb-pkg install <pacote>");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
