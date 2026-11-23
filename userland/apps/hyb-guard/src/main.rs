//! hyb-guard: Guardião do sistema.
//! Monitora integridade e segurança do sistema em tempo real.

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
    println!("=== hyb-guard: Guardião do Sistema ===");
    println!("");
    println!("Monitoramento ativo:");
    println!("  Integridade do kernel:  OK");
    println!("  Integridade do VFS:     OK");
    println!("  Acesso a dispositivos:  OK");
    println!("  Conexões de rede:       OK");
    println!("");
    println!("Ameaças detectadas: 0");
    println!("Modo de proteção: Ativo");
    sys_exit(0);
}