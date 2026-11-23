//! hyb-config-tree: Árvore de configuração.
//! Navega e edita a árvore de configuração do sistema.

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
    println!("=== hyb-config-tree: Árvore de Configuração ===");
    println!("");
    println!("/");
    println!("  ├── display/");
    println!("  ├── audio/");
    println!("  ├── input/");
    println!("  ├── power/");
    println!("  ├── network/");
    println!("  ├── security/");
    println!("  └── privacy/");
    println!("");
    println!("Use as setas para navegar, Enter para editar");
    sys_exit(0);
}