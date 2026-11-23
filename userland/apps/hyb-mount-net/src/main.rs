//! hyb-mount-net: Montagem de rede.
//! Monta compartilhamentos de rede (SMB, NFS, etc.).

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
    println!("=== hyb-mount-net: Montagem de Rede ===");
    println!("");
    println!("Compartilhamentos montados:");
    println!("  Nenhum compartilhamento montado");
    println!("");
    println!("Comandos:");
    println!("  hyb-mount-net <servidor> <compartilhamento> <ponto>");
    println!("  hyb-mount-net umount <ponto>");
    sys_exit(0);
}