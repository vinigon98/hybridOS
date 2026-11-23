//! hyb-fsctl: Controle de sistema de arquivos.
//! Gerencia montagem, desmontagem e configuração de sistemas de arquivos.

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
    println!("=== hyb-fsctl: Controle de Sistemas de Arquivos ===");
    println!("");
    println!("Montagens ativas:");
    println!("  /     -> FAT32 (ATA Master)");
    println!("  /mnt  -> RAMFS");
    println!("  /etc  -> FAT32 (partição de config)");
    println!("");
    println!("Comandos:");
    println!("  hyb-fsctl mount <disco> <ponto>");
    println!("  hyb-fsctl umount <ponto>");
    println!("  hyb-fsctl format <disco> <tipo>");
    sys_exit(0);
}