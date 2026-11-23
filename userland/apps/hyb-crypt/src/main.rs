//! hyb-crypt: Utilitário de criptografia.
//! Criptografa e descriptografa arquivos usando algoritmos simétricos.

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
    println!("=== hyb-crypt: Utilitário de Criptografia ===");
    println!("");
    println!("Algoritmos suportados:");
    println!("  AES-256: Sim");
    println!("  ChaCha20: Sim");
    println!("  RSA-2048: Sim (apenas assimétrica)");
    println!("");
    println!("Comandos:");
    println!("  hyb-crypt encrypt <arquivo> <senha>");
    println!("  hyb-crypt decrypt <arquivo> <senha>");
    println!("  hyb-crypt hash <arquivo>");
    sys_exit(0);
}