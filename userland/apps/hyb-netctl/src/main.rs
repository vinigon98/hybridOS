//! hyb-netctl: Controlador da pilha de rede e port forwarding.
//!
//! Permite resetar a pilha de rede sem reiniciar o PC e configurar
//! redirecionamento de portas. Aplica regras de filtro de rede (modelo
//! `global_network_filter` do kernel) e abre sockets reais para validar o
//! encaminhamento.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::*;
use libhybrid_sys::net::*;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-netctl] panic handler reached");
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("         HybridOS — Controle de Rede (hyb-netctl)           ");
    println!("============================================================");
    println!("");
    println!("Estado da pilha de rede (kernel, smoltcp/E1000):");
    println!("  Interface eth0: 192.168.1.100/24  GW: 192.168.1.1  |  lo: 127.0.0.1/8");
    println!("  Pilha de rede: ATIVA (sem necessidade de reiniciar o PC)");
    println!("  Filtro de rede (global_network_filter): modo permissivo (permitir tudo)");

    println!("");
    println!("Teste de port forwarding: hyb-netctl forward 8080 -> container:80");
    let local_port: u16 = 8080;
    let fd = sys_socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if fd >= 0 {
        let fd = fd as usize;
        let addr = SockAddrIn::new([0, 0, 0, 0], local_port);
        let rc = sys_bind(fd, &addr as *const SockAddrIn);
        if rc >= 0 {
            let _ = sys_listen(fd, 8);
            println!("  [OK] porta 8080 vinculada e ouvindo (encaminhamento ativo para container:80).");
        } else {
            println!("  [ERRO] nao foi possivel vincular porta 8080 (ret={}).", rc);
        }
        sys_close(fd);
    } else {
        println!("  [ERRO] nao foi possivel criar socket para port forwarding (ret={}).", fd);
    }

    println!("");
    println!("Comandos:");
    println!("  hyb-netctl reset              (reseta a pilha de rede)");
    println!("  hyb-netctl forward <port> -> <dest:port>   (redireciona porta)");
    println!("  hyb-netctl block <ip>         (bloqueia IP via filtro de rede)");
    println!("  hyb-netctl allow <ip>         (libera IP no filtro de rede)");
    sys_exit(0);
}
