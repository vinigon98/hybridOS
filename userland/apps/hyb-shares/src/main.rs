//! hyb-shares: Compartilhamento de rede.
//! Gerencia pastas compartilhadas e acesso à rede local.

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
    println!("=== hyb-shares: Compartilhamento de Rede ===");
    println!("");
    println!("Compartilhamentos ativos:");
    println!("  Nenhum compartilhamento ativo");
    println!("");
    println!("Comandos:");
    println!("  hyb-shares add <pasta> <nome>");
    println!("  hyb-shares remove <nome>");
    println!("  hyb-shares list");
    sys_exit(0);
}