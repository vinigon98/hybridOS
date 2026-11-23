//! hyb-image: Gerenciador de imagens de disco.
//! Cria, monta e gerencia imagens de disco virtuais.

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
    println!("=== hyb-image: Gerenciador de Imagens ===");
    println!("");
    println!("Imagens montadas:");
    println!("  Nenhuma imagem montada");
    println!("");
    println!("Formatos suportados: .img, .iso, .vhd");
    println!("Comandos:");
    println!("  hyb-image mount <arquivo> <ponto>");
    println!("  hyb-image umount <ponto>");
    println!("  hyb-image create <arquivo> <tamanho>");
    sys_exit(0);
}