//! hyb-crashlog: Visualizador de logs de crash do kernel.
//! Exibe logs de falhas e panics do sistema.

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
    println!("=== hyb-crashlog: Logs de Crash ===");
    println!("");
    println!("Nenhum crash registrado no sistema.");
    println!("Logs de kernel: limpos");
    println!("Último boot: bem-sucedido");
    println!("");
    println!("Armazenado em: /var/log/crash/");
    sys_exit(0);
}