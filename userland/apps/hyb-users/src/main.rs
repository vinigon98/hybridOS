//! hyb-users: Gerenciador de usuários.
//! Cria, edita e remove contas de usuário do sistema.

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
    println!("=== hyb-users: Contas de Usuário ===");
    println!("");
    println!("Usuário         Grupo      Shell");
    println!("-------------------------------");
    println!("root            administrador  /bin/hybridshell");
    println!("guest           visitante      /bin/hybridshell");
    println!("sysadmin        administrador  /bin/hybridshell");
    println!("");
    println!("Total: 3 contas de usuário");
    sys_exit(0);
}