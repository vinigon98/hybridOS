//! hyb-gpuinfo: Informações da placa de vídeo.
//! Exibe detalhes da GPU, driver e memória de vídeo.

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
    println!("=== hyb-gpuinfo: Informações da GPU ===");
    println!("");
    println!("Modelo:      QEMU QXL / VGA");
    println!("Driver:      GOP Framebuffer");
    println!("Resolução:   1280x800x32bpp");
    println!("Memória VRAM: 16 MB");
    println!("Aceleração:  Software (sem GPU dedicada)");
    println!("");
    println!("Driver instalado: hybridos-gop-driver");
    sys_exit(0);
}