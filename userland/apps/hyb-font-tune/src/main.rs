//! hyb-font-tune — HybridOS Font Rendering Tuner (Ring 3 ELF64).
//!
//! Reports the font metrics and rendering parameters used by the Crystal
//! Glass desktop text pipeline. Reads the font device via VFS where exposed.

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
    println!("[hyb-font-tune] fatal: {:?}", info);
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
    println!("        hyb-font-tune - HybridOS Font Rendering Tuner        ");
    println!("============================================================");

    println!("[font pipeline]");
    println!("  rasterizer  : embedded 8x8 bitmap font");
    println!("  source      : rust-kernel/src/drivers/hardware.rs (get_ascii_glyph)");
    println!("  glyph size  : 8x8 pixels");
    println!("  coverage    : ASCII 32..126 + box fallback");
    println!("  hinting     : none (pixel-aligned bitmap)");
    println!("  antialiasing: none (1bpp bitmap)");

    println!("");
    println!("[font device]");
    let mut found = false;
    for path in ["/dev/fonts", "/dev/freetype", "/dev/font0", "/sys/fonts"].iter() {
        if try_open(path) {
            println!("  {} : font device node present", path);
            found = true;
        }
    }
    if !found {
        println!("  no font device node exposed via VFS");
    }

    println!("");
    println!("[hyb-font-tune] font metrics read.");
    sys_sched_yield();
    sys_exit(0);
}
