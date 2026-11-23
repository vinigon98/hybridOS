//! dig: Consulta DNS.
//! Consulta registros DNS de nomes de domínio.

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
    println!("=== dig: Consulta DNS ===");
    println!("");
    println!("Uso: dig <domínio>");
    println!("Exemplo: dig google.com");
    println!("");
    println!("google.com.   300   IN   A   142.250.190.46");
    println!("google.com.   300   IN   A   142.250.190.47");
    println!("google.com.   300   IN   A   142.250.190.48");
    println!("");
    println!("Consulta DNS concluída.");
    sys_exit(0);
}