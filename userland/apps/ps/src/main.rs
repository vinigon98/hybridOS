//! ps — HybridOS Process Status (Ring 3 ELF64).
//!
//! Reads the LIVE kernel process table from `rust-kernel/src/sched/` through
//! the `sys_proc_list` (202) system call: PID, personality, status, name.
//! Also shows scheduler summary from `sys_sysinfo` (200).

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
    println!("[ps] fatal: {:?}", info);
    sys_exit(1);
}

const MAX_PROCS: usize = 64;

fn status_text(code: u32) -> &'static str {
    if code == 1 { "RUNNING" } else { "ACTIVE" }
}

fn personality_text(code: u32) -> &'static str {
    match code {
        1 => "Linux",
        2 => "WinNT",
        3 => "Native",
        4 => "Android",
        _ => "???",
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("                ps - HybridOS Process Status                 ");
    println!("============================================================");

    // REAL kernel scheduler summary from rust-kernel/src/sched/
    let mut info = SysInfo::default();
    let sysinfo_ok = unsafe { sys_sysinfo(&mut info) } >= 0;
    if sysinfo_ok {
        println!("[scheduler summary]");
        println!("  context switches: {}", info.context_switches);
        println!("  total ticks     : {}", info.total_ticks);
        println!("  uptime ticks    : {}", info.uptime_ticks);
        println!("  cpu count       : {}", info.cpu_count);
        println!("");
    }

    println!("[live process table (rust-kernel/src/sched/)]");
    println!("  PID    PERSONALITY  STATUS   NAME");
    println!("  ----------------------------------------------------------------");

    let mut entries = [ProcEntry::default(); MAX_PROCS];
    let ret = unsafe { sys_proc_list(entries.as_mut_ptr(), MAX_PROCS as u64) };
    let n = if ret < 0 { 0 } else { ret as usize };

    for i in 0..n {
        let e = &entries[i];
        let name = core::str::from_utf8(&e.name).unwrap_or("").trim_end_matches('\0');
        println!("  {:<6} {:<11} {:<8} {}", e.pid, personality_text(e.personality), status_text(e.status), name);
    }
    if n == 0 {
        println!("  (no process table returned by kernel)");
    }

    println!("");
    println!("[ps] {} process(es) listed from kernel scheduler tables.", n);
    sys_sched_yield();
    sys_exit(0);
}
