//! hyb-telemetry-local — HybridOS Local Telemetry Collector (Ring 3 ELF64).
//!
//! Reads LIVE kernel data from `rust-kernel/src/sched/` and `rust-kernel/src/mm/`
//! via `sys_sysinfo` (200) and emits a telemetry snapshot to stdout. Attempts to
//! append the snapshot to a local telemetry log through the VFS (sys_open/write).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-telemetry-local] fatal: {:?}", info);
    sys_exit(1);
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

fn append_log(path: &str, line: &str) {
    let fd = unsafe { sys_open(path, 0o100 | 0o2000, 0o644) }; // O_CREAT | O_APPEND
    if fd < 0 {
        return; // log not supported on this build: degrade gracefully
    }
    let bytes = line.as_bytes();
    let _ = unsafe { sys_write(fd as usize, bytes) };
    let _ = unsafe { sys_write(fd as usize, b"\n") };
    let _ = sys_close(fd as usize);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("   hyb-telemetry-local - HybridOS Local Telemetry Collector   ");
    println!("============================================================");

    let info = match read_sysinfo() {
        Some(i) => i,
        None => {
            println!("[telemetry] sys_sysinfo unavailable on this kernel build");
            sys_sched_yield();
            sys_exit(0);
        }
    };

    let used_pct = if info.total_mem_bytes == 0 {
        0
    } else {
        (info.used_mem_bytes * 100) / info.total_mem_bytes
    };

    let line = format!(
        "ticks={} ctx={} mem_used_kb={} mem_free_kb={} mem_use_pct={} frames={} heap_used_kb={}",
        info.uptime_ticks,
        info.context_switches,
        info.used_mem_bytes / 1024,
        info.free_mem_bytes / 1024,
        used_pct,
        info.free_frames,
        info.heap_used_bytes / 1024,
    );

    println!("[telemetry snapshot] {}", line);

    // Attempt to persist to a local telemetry log (VFS). Degrades gracefully.
    append_log("/var/log/telemetry.log", &line);
    append_log("/var/log/telemetry-local.log", &line);

    println!("[telemetry] snapshot written. Log append attempted (/var/log/telemetry*.log).");
    sys_sched_yield();
    sys_exit(0);
}
