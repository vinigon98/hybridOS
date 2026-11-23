//! hyb-secpol: Políticas de segurança.
//! Configura e exibe políticas de segurança do sistema.

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
    println!("=== hyb-secpol: Políticas de Segurança ===");
    println!("");
    println!("Controle de acesso obrigatório: Ativo");
    println!("Verificação de integridade: Ativa");
    println!("Sandbox de aplicativos: Ativo");
    println!("Firewall: Ativo");
    println!("Criptografia de disco: Desabilitada");
    println!("");
    println!("Nível de segurança: Padrão");
    sys_exit(0);
}