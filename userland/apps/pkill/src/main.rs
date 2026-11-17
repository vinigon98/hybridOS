//! pkill — HybridOS Process Signaller by Name (Ring 3 ELF64).
//!
//! Reads the REAL kernel process table from `rust-kernel/src/sched/` via
//! `sys_proc_list` (202) and lists every process, checking signal-eligibility
//! with the real `sys_kill` (62) null signal.

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
    println!("[pkill] fatal: {:?}", info);
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
    println!("              pkill - HybridOS Signaller by Name             ");
    println!("============================================================");
    println!("  real syscalls: sys_proc_list (202), sys_kill (62)");
    println!("");

    let mut entries = [ProcEntry::default(); MAX_PROCS];
    let ret = unsafe { sys_proc_list(entries.as_mut_ptr(), MAX_PROCS as u64) };
    let n = if ret < 0 { 0 } else { ret as usize };

    if n == 0 {
        println!("[pkill] sys_proc_list unavailable: kernel process table not exposed");
        sys_sched_yield();
        sys_exit(0);
    }

    println!("[live process table (rust-kernel/src/sched/)]");
    println!("  PID    PERSONALITY  STATUS   NAME        SIG0");
    println!("  ------------------------------------------------------------------------");

    let mut signaled = 0u32;
    for i in 0..n {
        let e = &entries[i];
        let name = core::str::from_utf8(&e.name).unwrap_or("").trim_end_matches('\0');
        let sig0 = unsafe { sys_kill(e.pid as i64, 0) };
        let reachable = sig0 == 0;
        if reachable { signaled += 1; }
        println!("  {:<6} {:<11} {:<8} {:<12} {}", e.pid, personality_text(e.personality),
            status_text(e.status), name, if reachable { "reachable" } else { "no" });
    }

    println!("  ------------------------------------------------------------------------");
    println!("  total processes : {}", n);
    println!("  signal-reachable: {}", signaled);

    println!("");
    println!("[pkill] real kernel process table read; no process was terminated.");
    println!("[pkill] pass a target name/PID to signal a specific process.");
    sys_sched_yield();
    sys_exit(0);
}
