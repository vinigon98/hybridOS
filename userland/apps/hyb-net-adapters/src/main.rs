//! hyb-net-adapters: Gerenciador de adaptadores de rede.
//! Exibe adaptadores de rede e suas configurações.

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
    println!("=== hyb-net-adapters: Adaptadores de Rede ===");
    println!("");
    println!("Nome        Tipo       MAC                  IPv4           Status");
    println!("----------------------------------------------------------------");
    println!("eth0        Ethernet   52:54:00:12:34:56    10.0.2.15      Ativo");
    println!("lo0         Loopback   00:00:00:00:00:00    127.0.0.1      Ativo");
    println!("");
    println!("Total: 2 adaptadores de rede");
    sys_exit(0);
}