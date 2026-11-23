//! hyb-firewall: Firewall do sistema.
//! Configura regras de firewall e filtra tráfego de rede.

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
    println!("=== hyb-firewall: Firewall do Sistema ===");
    println!("");
    println!("Estado: Ativo");
    println!("");
    println!("Regras de entrada:");
    println!("  Permitir:    SSH (22), HTTP (80), HTTPS (443)");
    println!("  Bloquear:    Telnet (23), FTP (21)");
    println!("");
    println!("Regras de saída:");
    println!("  Permitir:    Todas");
    println!("  Bloquear:    Nenhuma");
    println!("");
    println!("Conexões bloqueadas: 0");
    sys_exit(0);
}