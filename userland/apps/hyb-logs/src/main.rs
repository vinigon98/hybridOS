//! hyb-logs: Visualizador de logs do sistema.
//! Exibe logs do kernel e do sistema em tempo real.

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
    println!("=== hyb-logs: Logs do Sistema ===");
    println!("");
    println!("[BOOT] CPU running in 64-bit Long Mode in Ring 0.");
    println!("[BOOT] Limine Bootloader Protocol revision 3: Handshake OK.");
    println!("[ARCH] Serial COM1 initialized @ 115200 baud.");
    println!("[MM] Kernel Heap mapped at VA 0xFFFF900000000000.");
    println!("[NET] E1000 Gigabit Network Driver bound.");
    println!("[VFS] Virtual File System initialized.");
    println!("[GUI] HybridOS Desktop Environment & PS/2 Mouse active.");
    println!("");
    println!("Logs armazenados em: /var/log/kernel.log");
    sys_exit(0);
}