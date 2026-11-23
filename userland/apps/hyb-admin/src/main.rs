//! hyb-admin: Administrador do sistema.
//! Ferramentas de administração e manutenção do HybridOS.

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
    println!("=== hyb-admin: Administrador do Sistema ===");
    println!("");
    println!("Funções disponíveis:");
    println!("  - Gerenciar usuários");
    println!("  - Configurar serviços");
    println!("  - Manutenção do sistema");
    println!("  - Logs de auditoria");
    println!("");
    println!("Execute com privilégios de administrador.");
    sys_exit(0);
}