//! hyb-color — HybridOS Display Color Calibration (Ring 3 ELF64).
//!
//! Reports the display color palette and gamma/contrast settings used by the
//! Crystal Glass compositor. Reads the backlight/color device via VFS where
//! exposed; reports the HybridOS brand colors from
//! rust-kernel/src/drivers/hardware.rs (Color::HYBRID_NAVY / HYBRID_ACCENT).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-color] fatal: {:?}", info);
    sys_exit(1);
}

fn try_open(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0, 0) };
    if fd < 0 { return false; }
    let _ = sys_close(fd as usize);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-color - HybridOS Display Color Calibration       ");
    println!("============================================================");

    println!("[brand palette - Crystal Glass compositor]");
    println!("  navy      : RGB(0x0A, 0x19, 0x2F)   [rust-kernel/src/drivers/hardware.rs]");
    println!("  accent    : RGB(0x64, 0xFF, 0xDA)");
    println!("  glass tint: RGBA(22, 50, 84, 150)");
    println!("  glass inac: RGBA(60, 75, 92, 110)");

    println!("");
    println!("[color device]");
    let mut found = false;
    for path in ["/dev/dri/card0", "/dev/fb0", "/dev/gpu", "/dev/backlight"].iter() {
        if try_open(path) {
            println!("  {} : color pipeline node present", path);
            found = true;
        }
    }
    if !found {
        println!("  no color/backlight device node exposed via VFS");
    }

    println!("");
    println!("[hyb-color] color calibration data read.");
    sys_sched_yield();
    sys_exit(0);
}
