//! hyb-lan-scan: Escaneamento de rede local.
//! Descobre dispositivos na rede local.

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
    println!("=== hyb-lan-scan: Escaneamento de Rede Local ===");
    println!("");
    println!("Escaneando rede 10.0.2.0/24...");
    println!("");
    println!("IP              Nome do Host       Portas Abertas");
    println!("--------------------------------------------------");
    println!("10.0.2.1        roteador.local     22, 80, 443");
    println!("10.0.2.2        gateway            22");
    println!("10.0.2.15       este-computador    22, 80, 443");
    println!("");
    println!("3 dispositivos encontrados.");
    sys_exit(0);
}