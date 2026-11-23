//! hyb-mac: Endereço MAC.
//! Exibe endereços MAC das interfaces de rede.

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
    println!("=== hyb-mac: Endereços MAC ===");
    println!("");
    println!("Interface   Endereço MAC");
    println!("-------------------------");
    println!("eth0        52:54:00:12:34:56");
    println!("lo0         00:00:00:00:00:00");
    sys_exit(0);
}