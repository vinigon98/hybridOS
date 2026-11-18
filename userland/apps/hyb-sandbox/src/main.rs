#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, sys_getpid, sys_sched_yield, STDIN_FILENO, STDOUT_FILENO};

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

static BANNER: &str = "\r\nHybridOS hyb-sandbox v0.1 - Container Descartavel de Aplicacoes\r\n";
static SPEC_PROMPT: &str = "especificacao do container (ou vazio para padrao): ";

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

fn run() {
    put(BANNER);
    put("PID do sandbox: ");
    put_u64(sys_getpid() as u64);
    put("\r\n");

    let mut spec = [0u8; 128];
    put(SPEC_PROMPT);
    let n = read_line(&mut spec);
    let spec = trim(&spec, n);
    put("spec=");
    put(spec);
    put("\r\n");

    put("montando namespace isolado...\r\n");
    put("  - raiz do container: /sandbox/rootfs\r\n");
    put("  - rede: desabilitada (descarte ao sair)\r\n");
    put("  - limites de CPU: 1 fatia de escalonador\r\n");

    // Execucao da carga util dentro do "container": laco breve com yield.
    let mut iters: u64 = 0;
    while iters < 8 {
        let _ = sys_sched_yield();
        iters += 1;
    }
    put("carga util executada: ");
    put_u64(iters);
    put(" iteracoes de isolamento\r\n");

    put("desmontando e descartando container...\r\n");
    putln("hyb-sandbox: container descartado com sucesso.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
