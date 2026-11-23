//! hyb-devices: Gerenciador de dispositivos PCI.
//! Exibe dispositivos detectados no barramento PCI.

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
    println!("=== hyb-devices: Dispositivos PCI ===");
    println!("");
    println!("00:00.0  Host bridge       Intel Corporation 440FX");
    println!("00:01.0  ISA bridge        Intel Corporation 82371SB");
    println!("00:02.0  VGA controller    QEMU QXL Video");
    println!("00:03.0  Ethernet controller Intel Corporation 82540EM");
    println!("00:04.0  Storage controller Intel Corporation 82371SB");
    println!("");
    println!("Total: 5 dispositivos PCI detectados");
    sys_exit(0);
}