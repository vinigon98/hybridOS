//! hyb-bundle: Gerenciador de pacotes.
//! Gerencia instalação e remoção de pacotes de software.

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
    println!("=== hyb-bundle: Gerenciador de Pacotes ===");
    println!("");
    println!("Pacotes instalados:");
    println!("  hyb-files      0.1.0");
    println!("  hyb-edit       0.1.0");
    println!("  hyb-calc       0.1.0");
    println!("  ...");
    println!("");
    println!("Comandos:");
    println!("  hyb-bundle install <pacote>");
    println!("  hyb-bundle remove <pacote>");
    println!("  hyb-bundle list");
    println!("  hyb-bundle update");
    sys_exit(0);
}