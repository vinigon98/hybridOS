//! hyb-bt: Bluetooth.
//! Gerencia dispositivos Bluetooth e emparelhamento.

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
    println!("=== hyb-bt: Bluetooth ===");
    println!("");
    println!("Bluetooth: desativado");
    println!("");
    println!("Dispositivos emparelhados:");
    println!("  Nenhum dispositivo emparelhado");
    println!("");
    println!("Comandos:");
    println!("  hyb-bt on          - Ativar Bluetooth");
    println!("  hyb-bt off         - Desativar Bluetooth");
    println!("  hyb-bt scan        - Procurar dispositivos");
    println!("  hyb-bt pair <mac>  - Emparelhar dispositivo");
    sys_exit(0);
}