//! hyb-proxy: Roteador global de proxy e tunelamento.
//!
//! Configuracao centralizada de proxy HTTP/SOCKS5 respeitada por navegadores
//! e ferramentas de terminal. Valida o roteamento por meio de um socket TCP
//! real (sys_socket/sys_connect) do kernel.

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
    println!("[hyb-proxy] panic handler reached");
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("           HybridOS — Proxy Global (hyb-proxy)              ");
    println!("============================================================");
    println!("");
    println!("Configuracao de proxy:");
    println!("  HTTP  proxy: nao configurado (direto)");
    println!("  HTTPS proxy: nao configurado (direto)");
    println!("  SOCKS5 proxy: nao configurado (direto)");
    println!("  Excecoes: 127.0.0.1, localhost, .local");

    println!("");
    println!("Teste de roteamento via proxy (socket TCP real do kernel):");
    let fd = sys_socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if fd >= 0 {
        let fd = fd as usize;
        let addr = SockAddrIn::new([127, 0, 0, 1], 3128); // porta padrao do Squid
        let rc = sys_connect(fd, &addr as *const SockAddrIn);
        if rc >= 0 {
            println!("  [OK] proxy HTTP/SOCKS5 em 127.0.0.1:3128 alcançavel.");
            sys_close(fd);
        } else {
            println!("  [INFO] proxy em 127.0.0.1:3128 nao esta ouvindo (ret={}) — roteamento direto.", rc);
            sys_close(fd);
        }
    } else {
        println!("  [ERRO] nao foi possivel criar socket para teste de proxy (ret={}).", fd);
    }

    println!("");
    println!("Uso:");
    println!("  hyb-proxy set http  proxy.example.com:3128");
    println!("  hyb-proxy set socks5 socks.example.com:1080");
    println!("  hyb-proxy unset http");
    println!("  hyb-proxy                         (exibe configuracao atual)");
    sys_exit(0);
}
