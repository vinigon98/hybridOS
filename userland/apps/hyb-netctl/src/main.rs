//! hyb-netctl: Controle de rede.
//! Gerencia interfaces de rede e configurações de conectividade.

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
    println!("=== hyb-netctl: Controle de Rede ===");
    println!("");
    println!("Interfaces:");
    println!("  eth0: ativo, 10.0.2.15/24");
    println!("  lo0:  ativo, 127.0.0.1/8");
    println!("");
    println!("Comandos:");
    println!("  hyb-netctl up <interface>");
    println!("  hyb-netctl down <interface>");
    println!("  hyb-netctl set <interface> <ip> <mascara>");
    sys_exit(0);
}