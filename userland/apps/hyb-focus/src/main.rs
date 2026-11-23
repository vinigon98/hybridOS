//! hyb-focus: Modo de foco / concentração.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-focus: Modo de Foco ===");
    println!("");
    println!("Estado: INATIVO");
    println!("");
    println!("Quando ativado, o Modo de Foco:");
    println!("  - Oculta notificações da bandeja do sistema");
    println!("  - Minimiza janelas não essenciais");
    println!("  - Desativa sons do sistema");
    println!("  - Exibe temporizador de concentração");
    println!("");
    println!("Comandos:");
    println!("  hyb-focus on   — ativar modo de foco");
    println!("  hyb-focus off  — desativar modo de foco");
    println!("  hyb-focus status — exibir status atual");
    sys_exit(0);
}