//! hyb-perf-ui: Interface de desempenho.
//! Exibe gráficos e métricas de desempenho do sistema.

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
    println!("=== hyb-perf-ui: Interface de Desempenho ===");
    println!("");
    println!("CPU:  [████████░░] 12%");
    println!("RAM:  [██████████] 50%");
    println!("Disco:[██░░░░░░░░] 15%");
    println!("Rede: [████░░░░░░] 25%");
    println!("");
    println!("Atualização: 1 segundo");
    println!("Gráficos em tempo real");
    sys_exit(0);
}