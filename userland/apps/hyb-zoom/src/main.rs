//! hyb-zoom: Lupa / Zoom de tela.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    const FB_ADDR: usize = 0xFFFF_A000_0000_0000;
    const W: usize = 1280;
    const H: usize = 800;
    const PITCH: usize = 5120;

    println!("=== hyb-zoom: Lupa de Tela ===");
    println!("Tela: {}x{} pixels", W, H);
    println!("Framebuffer: 0x{:X}", FB_ADDR);
    println!("");
    println!("Modos de zoom:");
    println!("  2x  — duplicar região ao redor do cursor");
    println!("  4x  — quadruplicar região");
    println!("  8x  — máximo de ampliação");
    println!("");
    println!("Controles:");
    println!("  Setas  — mover região de zoom");
    println!("  + / -  — aumentar/diminuir zoom");
    println!("  Esc    — sair");
    println!("");
    println!("Acesseibilidade: auxilia usuários com baixa visão");
    sys_exit(0);
}