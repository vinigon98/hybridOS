//! hyb-proxy: Configuração de proxy.
//! Configura proxy de rede para o sistema.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-proxy: Configuração de Proxy ===");
    println!("");
    println!("Proxy HTTP:  não configurado");
    println!("Proxy HTTPS: não configurado");
    println!("Proxy FTP:   não configurado");
    println!("Proxy SOCKS: não configurado");
    println!("");
    println!("Comandos:");
    println!("  hyb-proxy set http <servidor>:<porta>");
    println!("  hyb-proxy set https <servidor>:<porta>");
    println!("  hyb-proxy clear");
    sys_exit(0);
}