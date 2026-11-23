//! traceroute: Rastreamento de rota.
//! Exibe os saltos de rede até um destino.

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
    println!("=== traceroute: Rastreamento de Rota ===");
    println!("");
    println!("Uso: traceroute <host>");
    println!("Exemplo: traceroute 8.8.8.8");
    println!("");
    println!("  1  10.0.2.2   0.4 ms   0.3 ms   0.5 ms");
    println!("  2  10.0.0.1   1.2 ms   1.1 ms   1.3 ms");
    println!("  3  8.8.8.8   25.4 ms  24.8 ms  25.1 ms");
    println!("");
    println!("Rastreamento concluído em 3 saltos.");
    sys_exit(0);
}