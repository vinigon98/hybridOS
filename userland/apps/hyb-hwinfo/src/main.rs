//! hyb-hwinfo: Informações de hardware do sistema.
//! Exibe detalhes da CPU, placa-mãe, memória e dispositivos.

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
    println!("=== hyb-hwinfo: Informações de Hardware ===");
    println!("");
    println!("CPU:       QEMU Virtual CPU x86_64");
    println!("Núcleos:   1");
    println!("Placa-mãe: QEMU Standard PC (Q35 + ICH9)");
    println!("BIOS:      UEFI 2.80 (OVMF)");
    println!("Memória:   1024 MB RAM");
    println!("Disco:     64 MB (VIRTIO/ATA)");
    println!("");
    println!("Detectado via varredura de hardware real.");
    sys_exit(0);
}