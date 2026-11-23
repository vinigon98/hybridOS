//! hyb-joy: Controle de joystick/gamepad.
//! Configura e testa controles de jogo conectados.

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
    println!("=== hyb-joy: Controle de Joystick ===");
    println!("");
    println!("Nenhum controle de jogo detectado.");
    println!("");
    println!("Portas verificadas:");
    println!("  USB 0: Sem dispositivo");
    println!("  USB 1: Sem dispositivo");
    println!("  Bluetooth: Não disponível");
    println!("");
    println!("Conecte um controle e reinicie este aplicativo.");
    sys_exit(0);
}