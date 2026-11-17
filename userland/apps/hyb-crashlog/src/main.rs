//! hyb-crashlog — HybridOS Kernel Crash Log Viewer (Ring 3 ELF64).
//!
//! Reads the kernel crash log through the VFS (sys_open/sys_read on
//! /var/log/crashlog). Degrades gracefully when no crash records exist.

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
    println!("[hyb-crashlog] fatal: {:?}", info);
    sys_exit(1);
}

const LOG_PATHS: [&str; 3] = ["/var/log/crashlog", "/var/log/crash.log", "/proc/crashlog"];

fn dump_log(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0 /* O_RDONLY */, 0) };
    if fd < 0 {
        return false;
    }
    let mut buf = [0u8; 1024];
    let mut total = 0usize;
    loop {
        let ret = unsafe { sys_read(fd as usize, &mut buf) };
        if ret <= 0 {
            break;
        }
        let n = ret as usize;
        let slice = &buf[..n];
        let _ = print!("{}", core::str::from_utf8(slice).unwrap_or("<binary>"));
        total += n;
    }
    let _ = sys_close(fd as usize);
    println!("\n[hyb-crashlog] {} bytes read from {}", total, path);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-crashlog - HybridOS Kernel Crash Log Viewer      ");
    println!("============================================================");

    let mut found = false;
    for path in LOG_PATHS.iter() {
        if dump_log(path) {
            found = true;
            break;
        }
    }

    if !found {
        println!("[hyb-crashlog] no crash log found on this build");
        println!("[hyb-crashlog] (checked /var/log/crashlog, /var/log/crash.log, /proc/crashlog)");
        println!("[hyb-crashlog] kernel crash logging via VFS not exposed or no records present");
    }

    println!("");
    println!("[hyb-crashlog] done.");
    sys_sched_yield();
    sys_exit(0);
}
