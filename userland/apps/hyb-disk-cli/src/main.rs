//! hyb-disk-cli: Utilitário de disco em linha de comando.
//! Gerencia partições, formatação e verificações de disco.

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
    println!("=== hyb-disk-cli: Utilitário de Disco ===");
    println!("");
    println!("Comandos disponíveis:");
    println!("  hyb-disk-cli list       - Listar discos");
    println!("  hyb-disk-cli format     - Formatar partição");
    println!("  hyb-disk-cli check      - Verificar integridade");
    println!("  hyb-disk-cli resize     - Redimensionar partição");
    println!("");
    println!("Use: hyb-disk-cli <comando> <disco>");
    sys_exit(0);
}