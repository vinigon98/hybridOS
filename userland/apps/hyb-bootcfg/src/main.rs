//! hyb-bootcfg: Configuração de boot.
//! Configura opções de boot do Limine e do kernel.

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
    println!("=== hyb-bootcfg: Configuração de Boot ===");
    println!("");
    println!("Bootloader: Limine v3");
    println!("Entrada padrão: HybridOS Kernel");
    println!("Timeout: 5 segundos");
    println!("Modo: BIOS Legacy / UEFI");
    println!("");
    println!("Entradas de boot:");
    println!("  1. HybridOS Kernel (padrão)");
    println!("  2. HybridOS Safe Mode");
    println!("  3. Memtest");
    println!("");
    println!("Arquivo: limine.conf");
    sys_exit(0);
}