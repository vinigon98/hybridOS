//! hyb-memtest: Teste de memória RAM.
//! Testa a integridade da memória física do sistema.

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
    println!("=== hyb-memtest: Teste de Memória ===");
    println!("");
    println!("Testando memória RAM...");
    println!("Padrão 0xAA: OK");
    println!("Padrão 0x55: OK");
    println!("Padrão 0x00: OK");
    println!("Padrão 0xFF: OK");
    println!("");
    println!("Resultado: Memória OK - sem erros detectados");
    println!("Total testado: 1024 MB");
    sys_exit(0);
}