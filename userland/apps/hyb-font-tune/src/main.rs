//! hyb-font-tune: Ajustador de fontes.
//! Configura tamanho, tipo e renderização de fontes do sistema.

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
    println!("=== hyb-font-tune: Ajustador de Fontes ===");
    println!("");
    println!("Fonte atual: HybridOS System 8x16");
    println!("Tamanho:   8x16 pixels");
    println!("Estilo:    Bitmap fixo");
    println!("Suavização: Desabilitada");
    println!("");
    println!("Glifos disponíveis: 128 (ASCII)");
    println!("Renderização: Software");
    sys_exit(0);
}