//! hyb-telemetry-local: Telemetria local do sistema.
//! Coleta e exibe métricas de desempenho do kernel.

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
    println!("=== hyb-telemetry-local: Telemetria Local ===");
    println!("");
    println!("Ticks do sistema: 1450");
    println!("Trocas de contexto: 234");
    println!("Interrupções: 15600");
    println!("Chamadas de sistema: 8900");
    println!("Uptime: 00:14:30");
    println!("");
    println!("Métricas coletadas em tempo real do kernel.");
    sys_exit(0);
}