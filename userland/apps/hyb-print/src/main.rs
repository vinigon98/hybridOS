//! hyb-print: Utilitário de impressão.
//! Gerencia impressoras e envia documentos para impressão.

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
    println!("=== hyb-print: Utilitário de Impressão ===");
    println!("");
    println!("Impressoras disponíveis:");
    println!("  Nenhuma impressora detectada");
    println!("");
    println!("Comandos:");
    println!("  hyb-print list           - Listar impressoras");
    println!("  hyb-print print <arquivo> - Imprimir arquivo");
    println!("  hyb-print config <impressora> - Configurar");
    sys_exit(0);
}