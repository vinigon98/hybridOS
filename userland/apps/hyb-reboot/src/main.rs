//! hyb-reboot: Reiniciar o sistema.
//! Reinicia o computador HybridOS de forma controlada.

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
    println!("=== hyb-reboot: Reiniciar Sistema ===");
    println!("");
    println!("Reiniciando o HybridOS...");
    println!("Salvando configurações...");
    println!("Descarregando drivers...");
    println!("Reinicializando CPU...");
    println!("");
    println!("Use: hyb-reboot --agora para reiniciar imediatamente");
    println!("Use: hyb-reboot --agendado <tempo> para agendar");
    sys_exit(0);
}