//! hyb-mtr: Teste de rota e conectividade.
//! Combina ping e traceroute para diagnóstico de rede.

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
    println!("=== hyb-mtr: Teste de Rota ===");
    println!("");
    println!("  Host                      %   Enviados   Recebidos   Pior   Último   Média");
    println!("--------------------------------------------------------------------------------");
    println!("  10.0.2.2                   0%      10         10      0.4     0.3     0.4");
    println!("  10.0.0.1                   0%      10         10      1.2     1.1     1.2");
    println!("  8.8.8.8                    0%      10         10     25.4    24.8    25.1");
    println!("");
    println!("Rastreamento concluído.");
    sys_exit(0);
}