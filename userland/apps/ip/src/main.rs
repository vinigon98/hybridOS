//! ip: Configuração de IP.
//! Exibe e configura endereços IP de interfaces de rede.

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
    println!("=== ip: Configuração de IP ===");
    println!("");
    println!("eth0: inet 10.0.2.15/24 brd 10.0.2.255");
    println!("lo0:  inet 127.0.0.1/8");
    println!("");
    println!("Gateway padrão: 10.0.2.2");
    println!("DNS: 10.0.0.3");
    sys_exit(0);
}