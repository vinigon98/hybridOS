#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, sys_getpid, sys_open, sys_close, STDOUT_FILENO};

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

const O_WRONLY_CREAT_TRUNC: u32 = 0x01 | 0x40 | 0x200;
const MODE: u32 = 0o644;

fn run() {
    put("HybridOS hyb-runner v0.1 - Executor Silencioso em Background\r\n");
    put("inicializando execucao de carga util em segundo plano...\r\n");

    let pid = sys_getpid();
    put("pid do executor: ");
    put_u64(pid as u64);
    put("\r\n");

    // Simula execucao de tarefa silenciosa: escritas de progresso para um arquivo de estado.
    let estado: &[u8] = b"rodando\n";
    let mut escrito = 0usize;
    while escrito < estado.len() {
        let n = sys_write(1, &estado[escrito..]);
        if n <= 0 { break; }
        escrito += n as usize;
    }

    // Escreve o PID em um arquivo de controle para supervisao.
    let fd = sys_open("/var/run/hyb-runner.pid", O_WRONLY_CREAT_TRUNC, MODE);
    if fd >= 0 {
        let pid_txt: &[u8] = b"1234\n"; // demonstrativo do pid gerenciado
        let _ = sys_write(fd as usize, pid_txt);
        let _ = sys_close(fd as usize);
        putln("arquivo de controle gravado: /var/run/hyb-runner.pid");
    } else {
        put("nao foi possivel gravar arquivo de controle (rc=");
        put_u64(fd as u64);
        putln(")");
    }

    putln("hyb-runner: carga util executada em segundo plano.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
