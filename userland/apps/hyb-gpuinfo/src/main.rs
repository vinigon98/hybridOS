//! hyb-gpuinfo — HybridOS Graphics / GPU Information (Ring 3 ELF64).
//!
//! Reports real CPU/GPU-related identifiers via CPUID and attempts to read
//! the framebuffer/GOP display configuration through the VFS. Degrades
//! gracefully when no graphics device node is exposed.

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
    println!("[hyb-gpuinfo] fatal: {:?}", info);
    sys_exit(1);
}

fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let r = unsafe { core::arch::x86_64::__cpuid_count(leaf, subleaf) };
    (r.eax as u32, r.ebx as u32, r.ecx as u32, r.edx as u32)
}

fn try_fb(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0 /* O_RDONLY */, 0) };
    if fd < 0 {
        return false;
    }
    let _ = sys_close(fd as usize);
    println!("  framebuffer node : {} (opened)", path);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("          hyb-gpuinfo - HybridOS GPU / Graphics Info         ");
    println!("============================================================");

    // CPUID vendor (real hardware data)
    let (v1, v2, v3, _) = cpuid(0, 0);
    let vendor: [u8; 14] = [
        v1.to_ne_bytes()[0], v1.to_ne_bytes()[1], v1.to_ne_bytes()[2], v1.to_ne_bytes()[3],
        v3.to_ne_bytes()[0], v3.to_ne_bytes()[1], v3.to_ne_bytes()[2], v3.to_ne_bytes()[3],
        v2.to_ne_bytes()[0], v2.to_ne_bytes()[1], v2.to_ne_bytes()[2], v2.to_ne_bytes()[3],
        0, 0,
    ];
    println!("[cpu/vendor] {:?}", core::str::from_utf8(&vendor).unwrap_or(""));

    // Feature flags: check for common bits
    let (_, _, _, d) = cpuid(1, 0);
    let mmx = (d >> 23) & 1 == 1;
    let sse = (d >> 25) & 1 == 1;
    let sse2 = (d >> 26) & 1 == 1;
    let ht = (d >> 28) & 1 == 1;
    println!("[cpu features] mmx={} sse={} sse2={} ht={}", mmx, sse, sse2, ht);

    println!("");
    println!("[framebuffer / GOP]");
    let mut fb_found = false;
    for path in ["/dev/fb0", "/dev/fb", "/proc/fb", "/sys/class/graphics/fb0"].iter() {
        if try_fb(path) {
            fb_found = true;
            break;
        }
    }
    if !fb_found {
        println!("  no framebuffer device node exposed via VFS");
        println!("  (GOP framebuffer dims live in rust-kernel/src/drivers/hardware.rs)");
    }

    println!("");
    println!("[hyb-gpuinfo] done.");
    sys_sched_yield();
    sys_exit(0);
}
