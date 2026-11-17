//! hyb-perf-ui — HybridOS Performance UI (Ring 3 ELF64).
//!
//! Reads LIVE kernel performance data from `rust-kernel/src/sched/` and
//! `rust-kernel/src/mm/` via `sys_sysinfo` (200) and renders a performance
//! dashboard: scheduler stats, memory pressure, frame usage, heap usage.

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
    println!("[hyb-perf-ui] fatal: {:?}", info);
    sys_exit(1);
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

fn bar(used: u64, total: u64, width: usize) {
    let filled = if total == 0 { 0 } else { (used as usize) * width / (total as usize) };
    let filled = filled.min(width);
    print!("[");
    let mut i = 1usize;
    while i <= filled { print!("#"); i += 1; }
    while i <= width { print!("-"); i += 1; }
    print!("]");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("           hyb-perf-ui - HybridOS Performance UI             ");
    println!("============================================================");

    let info = match read_sysinfo() {
        Some(i) => i,
        None => {
            println!("[hyb-perf-ui] sys_sysinfo unavailable on this kernel build");
            sys_sched_yield();
            sys_exit(0);
        }
    };

    println!("[scheduler - rust-kernel/src/sched/]");
    println!("  context switches: {}", info.context_switches);
    println!("  total ticks     : {}", info.total_ticks);
    println!("  uptime ticks    : {}", info.uptime_ticks);
    println!("  cpu count       : {}", info.cpu_count);

    let total = info.total_mem_bytes;
    let used = info.used_mem_bytes;
    let pct = if total == 0 { 0 } else { (used * 100) / total };

    println!("");
    println!("[memory - rust-kernel/src/mm/]");
    print!("  ram pressure   : ");
    bar(used, total, 26);
    println!("  {}%%  ({}/{} KB)", pct, used / 1024, total / 1024);

    let frames_used = info.total_frames.saturating_sub(info.free_frames);
    print!("  frame usage    : ");
    bar(frames_used, info.total_frames, 26);
    let fpct = if info.total_frames == 0 { 0 } else { (frames_used * 100) / info.total_frames };
    println!("  {}%%  ({}/{} frames)", fpct, frames_used, info.total_frames);

    let heap_total = info.heap_used_bytes + info.heap_free_bytes;
    print!("  heap usage     : ");
    bar(info.heap_used_bytes, heap_total, 26);
    let hpct = if heap_total == 0 { 0 } else { (info.heap_used_bytes * 100) / heap_total };
    println!("  {}%%  ({}/{} KB)", hpct, info.heap_used_bytes / 1024, heap_total / 1024);

    println!("");
    println!("[hyb-perf-ui] performance snapshot complete (real kernel data).");
    sys_sched_yield();
    sys_exit(0);
}
