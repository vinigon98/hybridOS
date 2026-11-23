//! hyb-color: Gerenciador de cores do sistema.
//! Ajusta cores de temas, contraste e brilho da tela.

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
    println!("=== hyb-color: Gerenciador de Cores ===");
    println!("");
    println!("Tema atual: Cristalino (Sapphire)");
    println!("");
    println!("Cores do tema:");
    println!("  Fundo:      #0F172A");
    println!("  Texto:      #F8FAFC");
    println!("  Destaque:   #0284C7");
    println!("  Borda:      #475569");
    println!("");
    println!("Brilho da tela: 80%");
    println!("Contraste: Normal");
    sys_exit(0);
}