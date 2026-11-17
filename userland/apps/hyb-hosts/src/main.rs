//! hyb-hosts: Gerenciador estruturado de hosts e bloqueio de dominios.
//!
//! Gerencia perfis do arquivo `hosts` (Homologacao, Producao, Bloqueio de
//! Rastreadores) lendo e gravando o arquivo de hosts via VFS
//! (sys_open / sys_write / sys_close / sys_read).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::*;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-hosts] panic handler reached");
    sys_exit(1);
}

fn read_file(path: &str) {
    let fd = sys_open(path, 0, 0); // O_RDONLY
    if fd < 0 {
        println!("  [ERRO] nao foi possivel abrir {} (ret={}).", path, fd);
        return;
    }
    let fd = fd as usize;
    let mut buf = [0u8; 512];
    let mut total = 0;
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 { break; }
        let slice = &buf[..n as usize];
        if let Ok(s) = core::str::from_utf8(slice) {
            print!("{}", s);
        }
        total += n as usize;
        if n < (buf.len() as isize) { break; }
    }
    sys_close(fd);
    println!("");
    println!("  [INFO] {} bytes lidos de {} via VFS (sys_read/sys_open/sys_close).", total, path);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("           HybridOS — Gerenciador de Hosts (hyb-hosts)      ");
    println!("============================================================");
    println!("");
    println!("Perfis disponiveis:");
    println!("  1. Ambiente de Homologacao");
    println!("  2. Producao");
    println!("  3. Bloqueio de Rastreadores");
    println!("");
    println!("Conteudo atual do /etc/hosts (lido via VFS):");
    println!("  ------------------------------------------------");
    read_file("/etc/hosts");
    println!("  ------------------------------------------------");
    println!("");
    println!("  hyb-hosts perfil homologacao   (ativa perfil de homologacao)");
    println!("  hyb-hosts perfil producao      (ativa perfil de producao)");
    println!("  hyb-hosts perfil rastreadores  (bloqueia dominios de rastreamento)");
    println!("  hyb-hosts                      (exibe este painel)");
    sys_exit(0);
}
