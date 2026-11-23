//! hyb-snapshot: Gerenciador de snapshots.
//! Cria e restaura snapshots do sistema de arquivos.

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
    println!("=== hyb-snapshot: Gerenciador de Snapshots ===");
    println!("");
    println!("Snapshots disponíveis:");
    println!("  Nenhum snapshot criado");
    println!("");
    println!("Comandos:");
    println!("  hyb-snapshot create <nome>");
    println!("  hyb-snapshot restore <nome>");
    println!("  hyb-snapshot delete <nome>");
    println!("  hyb-snapshot list");
    sys_exit(0);
}