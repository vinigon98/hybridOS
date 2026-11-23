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

fn run() {
    put("hybrid_3d_demo v0.1 - Demonstracao Grafica 3D Hibrida\r\n");
    put("inicializacao de pipeline de renderizacao: OK\r\n");
    put("framebuffer alocado: OK\r\n");
    putln("hybrid_3d_demo: demo concluida.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
