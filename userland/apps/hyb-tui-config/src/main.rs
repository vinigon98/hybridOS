//! hyb-tui-config: Configuração em modo texto.
//! Interface de linha de comando para configurar o sistema.

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
    println!("=== hyb-tui-config: Configuração em Modo Texto ===");
    println!("");
    println!("Seções disponíveis:");
    println!("  1. Tela e Personalização");
    println!("  2. Rede e Internet");
    println!("  3. Hardware e Dispositivos");
    println!("  4. Armazenamento e Partições");
    println!("  5. Energia e CPU");
    println!("");
    println!("Use: hyb-tui-config <secao> para configurar");
    sys_exit(0);
}