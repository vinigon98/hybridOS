//! hyb-sudo: Executar comando como outro usuário.
//! Eleva privilégios para execução de comandos administrativos.

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
    println!("=== hyb-sudo: Executar como Administrador ===");
    println!("");
    println!("Uso: hyb-sudo <comando>");
    println!("Exemplo: hyb-sudo hyb-disks");
    println!("");
    println!("Autenticação necessária para comandos administrativos.");
    println!("Digite a senha de administrador quando solicitado.");
    sys_exit(0);
}