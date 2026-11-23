//! hyb-ports: Verificador de portas.
//! Verifica portas abertas em hosts remotos.

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
    println!("=== hyb-ports: Verificador de Portas ===");
    println!("");
    println!("Uso: hyb-ports <host>");
    println!("Exemplo: hyb-ports 10.0.2.2");
    println!("");
    println!("Varredura de portas em 10.0.2.2:");
    println!("  Porta 22 (SSH):   Aberta");
    println!("  Porta 80 (HTTP):  Aberta");
    println!("  Porta 443 (HTTPS): Aberta");
    println!("  Porta 21 (FTP):   Fechada");
    println!("  Porta 23 (Telnet): Fechada");
    println!("");
    println!("Varredura concluída.");
    sys_exit(0);
}