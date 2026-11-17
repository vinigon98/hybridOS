//! hyb-logs — HybridOS Kernel Log Reader (Ring 3 ELF64).
//!
//! Reads kernel logs through the VFS (sys_open/sys_read on
//! /var/log/kernel.log, /proc/kmsg, /var/log/syslog). Degrades gracefully
//! when no log backend is exposed.

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
    println!("[hyb-logs] fatal: {:?}", info);
    sys_exit(1);
}

const LOG_PATHS: [&str; 4] = [
    "/var/log/kernel.log",
    "/proc/kmsg",
    "/var/log/syslog",
    "/var/log/messages",
];

fn dump_log(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0, 0) };
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
    println!("\n[hyb-logs] {} bytes read from {}", total, path);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("           hyb-logs - HybridOS Kernel Log Reader             ");
    println!("============================================================");

    let mut found = false;
    for path in LOG_PATHS.iter() {
        if dump_log(path) {
            found = true;
            break;
        }
    }

    if !found {
        println!("[hyb-logs] no kernel log exposed via VFS on this build");
        println!("[hyb-logs] (checked /var/log/kernel.log, /proc/kmsg, /var/log/syslog, /var/log/messages)");
    }

    println!("");
    println!("[hyb-logs] done.");
    sys_sched_yield();
    sys_exit(0);
}
