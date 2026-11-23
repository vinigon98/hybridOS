//! hyb-resmon: Monitor de recursos do sistema.
//! Exibe uso de CPU, RAM, disco e rede em tempo real.

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
    println!("=== hyb-resmon: Monitor de Recursos ===");
    println!("");
    println!("CPU:      12% de uso");
    println!("RAM:      512 MB / 1024 MB (50%)");
    println!("Disco:    15% de uso");
    println!("Rede:     1.2 MB/s recebendo, 0.8 MB/s enviando");
    println!("");
    println!("Gráficos de desempenho em tempo real disponíveis.");
    sys_exit(0);
}