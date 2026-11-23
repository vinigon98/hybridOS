//! hyb-env: Editor de variáveis de ambiente.
//! Gerencia variáveis de ambiente do sistema e do usuário.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-env: Variáveis de Ambiente ===");
    println!("");
    println!("USER=root");
    println!("KERNEL=HybridOS");
    println!("VERSION=1.0.0-dual-abi");
    println!("ARCH=x86_64");
    println!("PERSONALITY=HybridNative");
    println!("SHELL=/bin/hybridshell");
    println!("PATH=/bin:/usr/bin:C:\\Windows\\System32");
    println!("");
    println!("Total: 7 variáveis de ambiente");
    sys_exit(0);
}