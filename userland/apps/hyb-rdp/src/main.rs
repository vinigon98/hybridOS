//! hyb-rdp: Cliente de conexão remota (TCP).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-rdp: Cliente Remoto TCP ===");
    println!("Uso: hyb-rdp <ip> <porta>");
    println!("Exemplo: hyb-rdp 10.0.2.2 3389");
    println!("");
    println!("Estabelece conexão TCP com host remoto");
    println!("e exibe dados recebidos no console.");
    println!("");
    println!("Para sair: pressione Ctrl+C ou digite 'quit'");
    sys_exit(0);
}