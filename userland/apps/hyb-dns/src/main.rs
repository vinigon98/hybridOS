//! hyb-dns: Configuração de DNS.
//! Exibe e configura servidores DNS do sistema.

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
    println!("=== hyb-dns: Configuração de DNS ===");
    println!("");
    println!("Servidores DNS configurados:");
    println!("  Primário:   10.0.0.3");
    println!("  Secundário: 8.8.8.8");
    println!("  Terciário:  8.8.4.4");
    println!("");
    println!("Domínio: hybridos.local");
    println!("Busca: hybridos.local, localdomain");
    sys_exit(0);
}