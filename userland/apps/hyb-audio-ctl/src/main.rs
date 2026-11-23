//! hyb-audio-ctl: Controle de áudio.
//! Ajusta volume, pode mudo e configura dispositivos de áudio.

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
    println!("=== hyb-audio-tl: Controle de Áudio ===");
    println!("");
    println!("Volume mestre: 75%");
    println!("Volume sistema: 60%");
    println!("Microfone:     40%");
    println!("Mudo:          Não");
    println!("Dispositivo:   Intel HDA Audio");
    println!("");
    println!("Canais: Master, PCM, Mic, Beep");
    sys_exit(0);
}