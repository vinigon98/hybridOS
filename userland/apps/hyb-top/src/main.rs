//! hyb-top — Real-time process & CPU monitor (Ring 3 ELF64).
//!
//! Reads LIVE kernel data from `rust-kernel/src/sched/` and `rust-kernel/src/mm/`
//! through the `sys_sysinfo` (200) and `sys_proc_list` (202) system calls:
//!   - scheduler: context switches, total ticks, uptime ticks
//!   - memory:    total/free/used RAM, frames, kernel heap
//!   - processes: live process table (PID, personality, status, name)

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::println;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-top] fatal: {:?}", info);
    sys_exit(1);
}

const MAX_PROCS: usize = 64;

fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let r = unsafe { core::arch::x86_64::__cpuid_count(leaf, subleaf) };
    (r.eax as u32, r.ebx as u32, r.ecx as u32, r.edx as u32)
}

fn cpu_brand_string() -> [u8; 48] {
    let mut brand = [0u8; 48];
    let (_, b, _, d) = cpuid(0, 0);
    let max_leaf = b.max(d);
    if max_leaf >= 0x80000004 {
        let mut offset = 0;
        for leaf in 0x80000002u32..=0x80000004u32 {
            let (a, b, c, d) = cpuid(leaf, 0);
            for v in [a, b, c, d] {
                let bytes = v.to_ne_bytes();
                for &b in &bytes {
                    if offset < 48 && b != 0 {
                        brand[offset] = b;
                    }
                    offset += 1;
                }
            }
        }
    }
    brand
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = sys_sysinfo(&mut info);
    if ret < 0 {
        None
    } else {
        Some(info)
    }
}

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
    println!("        hyb-top - HybridOS Live Process / CPU Monitor       ");
    println!("============================================================");

    let info = read_sysinfo();

    if let Some(m) = &info {
        println!("[kernel mm/sched snapshot]");
        println!("  total RAM     : {} KB", m.total_mem_bytes / 1024);
        println!("  free RAM      : {} KB", m.free_mem_bytes / 1024);
        println!("  used RAM      : {} KB", m.used_mem_bytes / 1024);
        println!("  frames total  : {}", m.total_frames);
        println!("  frames free   : {}", m.free_frames);
        println!("  heap used     : {} KB", m.heap_used_bytes / 1024);
        println!("  heap free     : {} KB", m.heap_free_bytes / 1024);
        println!("  ctx switches  : {}", m.context_switches);
        println!("  total ticks   : {}", m.total_ticks);
        println!("  uptime ticks  : {}", m.uptime_ticks);
        println!("  cpu count     : {}", m.cpu_count);
    } else {
        println!("[kernel mm/sched snapshot] sys_sysinfo unavailable on this build");
    }

    // CPU brand via CPUID (real hardware data)
    let (v1, v2, v3, _) = cpuid(0, 0);
    let vendor: [u8; 12] = [
        v1.to_ne_bytes()[0], v1.to_ne_bytes()[1], v1.to_ne_bytes()[2], v1.to_ne_bytes()[3],
        v3.to_ne_bytes()[0], v3.to_ne_bytes()[1], v3.to_ne_bytes()[2], v3.to_ne_bytes()[3],
        v2.to_ne_bytes()[0], v2.to_ne_bytes()[1], v2.to_ne_bytes()[2], v2.to_ne_bytes()[3],
    ];
    let brand = cpu_brand_string();
    let brand_str = core::str::from_utf8(&brand).unwrap_or("");
    println!("[cpu] vendor: {:?} | brand: {}", core::str::from_utf8(&vendor).unwrap_or(""), brand_str);

    println!("");
    println!("live process table (from rust-kernel/src/sched/):");
    println!("  PID    PERSONALITY  STATUS   NAME");
    println!("  ----------------------------------------------------------------");

    let mut entries = [ProcEntry::default(); MAX_PROCS];
    let ret = sys_proc_list(entries.as_mut_ptr(), MAX_PROCS as u64);
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
    println!("[hyb-top] sampling complete. Yielding to scheduler.");
    sys_sched_yield();
    sys_exit(0);
}
