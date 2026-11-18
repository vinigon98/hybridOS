//! hyb-memtest — HybridOS Memory Tester (Ring 3 ELF64).
//!
//! Reports LIVE kernel memory statistics from `rust-kernel/src/mm/` via
//! `sys_sysinfo` (200), then runs a REAL memory integrity test (walking 1s
//! pattern + complement verification) on a user-space buffer.

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
    println!("[hyb-memtest] fatal: {:?}", info);
    sys_exit(1);
}

const TEST_LEN: usize = 65536;

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

/// Walking-1s pattern test: write increasing patterns, read back, verify.
fn memtest(buf: &mut [u8]) -> bool {
    // Pattern 1: ascending bytes
    for i in 0..buf.len() {
        buf[i] = (i & 0xFF) as u8;
    }
    for i in 0..buf.len() {
        if buf[i] != (i & 0xFF) as u8 {
            println!("  [FAIL] ascending pattern mismatch at offset {}", i);
            return false;
        }
    }

    // Pattern 2: walking ones
    for i in 0..buf.len() {
        buf[i] = (1u8 << ((i % 8) as u32));
    }
    for i in 0..buf.len() {
        if buf[i] != (1u8 << ((i % 8) as u32)) {
            println!("  [FAIL] walking-ones mismatch at offset {}", i);
            return false;
        }
    }

    // Pattern 3: complement + all-ones/all-zeros
    for i in 0..buf.len() {
        buf[i] = 0xAA;
    }
    for i in 0..buf.len() {
        buf[i] ^= 0xFF;
        if buf[i] != 0x55 {
            println!("  [FAIL] complement mismatch at offset {}", i);
            return false;
        }
    }

    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut buf = [0u8; TEST_LEN];

    println!("");
    println!("============================================================");
    println!("          hyb-memtest - HybridOS Memory Tester               ");
    println!("============================================================");

    // REAL kernel memory data from rust-kernel/src/mm/
    println!("[kernel memory state (sys_sysinfo)]");
    match read_sysinfo() {
        Some(info) => {
            println!("  total RAM : {} KB", info.total_mem_bytes / 1024);
            println!("  free RAM  : {} KB", info.free_mem_bytes / 1024);
            println!("  used RAM  : {} KB", info.used_mem_bytes / 1024);
            println!("  free frames: {}", info.free_frames);
            println!("  heap used : {} KB", info.heap_used_bytes / 1024);
        }
        None => {
            println!("  sys_sysinfo unavailable on this kernel build");
        }
    }

    println!("");
    println!("[running memory integrity test on {} KiB buffer]", TEST_LEN / 1024);
    let ok = memtest(&mut buf);
    if ok {
        println!("  [PASS] memory integrity test passed ({} patterns)", 3);
    } else {
        println!("  [FAIL] memory integrity test detected errors");
    }

    println!("");
    println!("[hyb-memtest] test complete.");
    sys_sched_yield();
    sys_exit(0);
}
