//! hyb-ip: Informações de IP.
//! Exibe detalhes de endereçamento IP de interfaces.

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
    println!("=== hyb-ip: Informações de IP ===");
    println!("");
    println!("eth0:");
    println!("  endereço: 10.0.2.15");
    println!("  máscara:  255.255.255.0");
    println!("  rede:     10.0.2.0");
    println!("  broadcast: 10.0.2.255");
    println!("");
    println!("lo0:");
    println!("  endereço: 127.0.0.1");
    println!("  máscara:  255.0.0.0");
    sys_exit(0);
}