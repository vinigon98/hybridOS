//! kill: Encerrar processo.
//! Envia sinal de término para um processo pelo PID.

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
    println!("=== kill: Encerrar Processo ===");
    println!("");
    println!("Uso: kill <PID>");
    println!("Exemplo: kill 102");
    println!("");
    println!("Sinais disponíveis:");
    println!("  SIGTERM (15) - Encerramento gracioso");
    println!("  SIGKILL (9)  - Encerramento forçado");
    println!("");
    println!("Nenhum PID especificado. Use: kill <numero>");
    sys_exit(0);
}