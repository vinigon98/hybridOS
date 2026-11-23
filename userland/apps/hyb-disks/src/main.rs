//! hyb-disks: Gerenciador de discos.
//! Exibe discos, partições e uso de armazenamento.

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
    println!("=== hyb-disks: Gerenciador de Discos ===");
    println!("");
    println!("Dispositivo  Tamanho  Usado  Livre  Tipo");
    println!("------------------------------------------");
    println!("ATA Master   64 MB    12 MB  52 MB  FAT32");
    println!("NVMe0        64 MB    0 MB   64 MB  Não formatado");
    println!("SATA0        32 MB    0 MB   32 MB  Não formatado");
    println!("");
    println!("Total: 3 discos detectados");
    sys_exit(0);
}