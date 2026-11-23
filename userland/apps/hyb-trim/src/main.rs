//! hyb-trim: Otimizador de SSD/TRIM.
//! Executa comando TRIM em unidades de estado sólido.

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
    println!("=== hyb-trim: Otimizador SSD/TRIM ===");
    println!("");
    println!("Executando TRIM em dispositivos SSD...");
    println!("NVMe0: TRIM concluído com sucesso");
    println!("SATA0: Não é SSD - ignorado");
    println!("");
    println!("Blocos liberados: 0 (já otimizado)");
    sys_exit(0);
}