//! hyb-fonts: Visualizador de fontes.

#![no_std]
#![no_main]

use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-fonts: Visualizador de Fontes ===");
    println!("");
    println!("Fonte padrão do sistema:");
    println!("  Nome:        HybridOS System Font");
    println!("  Tamanho:     8x16 pixels");
    println!("  Estilo:      Bitmap fixo (monoespaçado)");
    println!("  Glifos:      128 (ASCII padrão)");
    println!("  Codificação: ASCII / UTF-8");
    println!("");
    println!("Faixas de glifos disponíveis:");
    println!("  0x20-0x7E : ASCII impressível (95 glifos)");
    println!("  0x00-0x1F : Caracteres de controle");
    println!("  0x7F      : DEL");
    println!("");
    println!("Renderização: software (framebuffer GOP)");
    sys_exit(0);
}
