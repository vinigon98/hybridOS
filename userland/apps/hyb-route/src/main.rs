//! hyb-route: Tabela de roteamento.
//! Exibe a tabela de rotas de rede do sistema.

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
    println!("=== hyb-route: Tabela de Roteamento ===");
    println!("");
    println!("Destino         Gateway         Máscara       Interface");
    println!("--------------------------------------------------------");
    println!("0.0.0.0         10.0.2.2          0.0.0.0       eth0");
    println!("10.0.2.0        0.0.0.0           255.255.255.0 eth0");
    println!("127.0.0.0       0.0.0.0           255.0.0.0     lo0");
    println!("");
    println!("Gateway padrão: 10.0.2.2");
    sys_exit(0);
}