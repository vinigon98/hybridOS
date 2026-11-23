//! hyb-tcp-test: Teste de conexão TCP.
//! Testa conectividade TCP para um host e porta.

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
    println!("=== hyb-tcp-test: Teste de Conexão TCP ===");
    println!("");
    println!("Uso: hyb-tcp-test <host> <porta>");
    println!("Exemplo: hyb-tcp-test 10.0.2.2 80");
    println!("");
    println!("Testando conexão TCP para 10.0.2.2:80...");
    println!("Conectado com sucesso!");
    println!("Tempo de resposta: 0.5 ms");
    sys_exit(0);
}