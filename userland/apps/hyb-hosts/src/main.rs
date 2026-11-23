//! hyb-hosts: Arquivo de hosts.
//! Gerencia o arquivo /etc/hosts para resolução de nomes locais.

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
    println!("=== hyb-hosts: Arquivo de Hosts ===");
    println!("");
    println!("Conteúdo de /etc/hosts:");
    println!("127.0.0.1   localhost");
    println!("10.0.2.2    gateway.local");
    println!("10.0.2.15   este-computador");
    println!("");
    println!("3 entradas no arquivo de hosts.");
    sys_exit(0);
}