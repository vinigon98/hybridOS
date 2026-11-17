//! hyb-resmon — HybridOS Resource Monitor (Ring 3 ELF64).
//!
//! Reads LIVE kernel memory data from `rust-kernel/src/mm/` through the
//! `sys_sysinfo` (200) system call:
//!   - total / free / used physical RAM
//!   - total / free / allocated frames
//!   - kernel dynamic heap usage

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
    println!("[hyb-resmon] fatal: {:?}", info);
    sys_exit(1);
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

fn print_gauge(used: u64, total: u64, width: usize) {
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
    println!("        hyb-resmon - HybridOS Kernel Resource Monitor       ");
    println!("============================================================");

    let info = match read_sysinfo() {
        Some(i) => i,
        None => {
            println!("[hyb-resmon] sys_sysinfo unavailable on this kernel build");
            println!("[hyb-resmon] cannot display real memory data without kernel MM syscall");
            sys_sched_yield();
            sys_exit(0);
        }
    };

    let total = info.total_mem_bytes;
    let free = info.free_mem_bytes;
    let used = info.used_mem_bytes;
    let pct = if total == 0 { 0 } else { (used * 100) / total };

    println!("[physical memory - real data from rust-kernel/src/mm/]");
    println!("  total : {} KB  ({} MB)", total / 1024, total / (1024 * 1024));
    println!("  used  : {} KB  ({}%%)", used / 1024, pct);
    println!("  free  : {} KB", free / 1024);
    print!("  gauge : ");
    print_gauge(used, total, 30);
    println!("  {}/{} KB", used / 1024, total / 1024);

    println!("");
    println!("[physical frames]");
    println!("  total frames: {}", info.total_frames);
    println!("  free frames : {}", info.free_frames);
    println!("  used frames : {}", info.total_frames.saturating_sub(info.free_frames));

    println!("");
    println!("[kernel dynamic heap]");
    println!("  used : {} KB", info.heap_used_bytes / 1024);
    println!("  free : {} KB", info.heap_free_bytes / 1024);

    let heap_total = info.heap_used_bytes + info.heap_free_bytes;
    let hpct = if heap_total == 0 { 0 } else { (info.heap_used_bytes * 100) / heap_total };
    print!("  gauge : ");
    print_gauge(info.heap_used_bytes, heap_total, 30);
    println!("  {}%%", hpct);

    println!("");
    println!("[scheduler summary]");
    println!("  context switches: {}", info.context_switches);
    println!("  total ticks     : {}", info.total_ticks);
    println!("  uptime ticks    : {}", info.uptime_ticks);

    println!("");
    println!("[hyb-resmon] memory sampling complete.");
    sys_sched_yield();
    sys_exit(0);
}
