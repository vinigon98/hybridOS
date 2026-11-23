//! hyb-mixer: Mixer de volume.
//! Controle individual de volume por aplicativo.

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
    println!("=== hyb-mixer: Mixer de Volume ===");
    println!("");
    println!("Canal        Volume  Mudo");
    println!("--------------------------");
    println!("Master       75%     Não");
    println!("Sistema      60%     Não");
    println!("Wave         80%     Não");
    println!("Synth        50%     Sim");
    println!("CD           0%      Sim");
    println!("Microfone    40%     Não");
    println!("");
    println!("Use sliders para ajustar cada canal.");
    sys_exit(0);
}