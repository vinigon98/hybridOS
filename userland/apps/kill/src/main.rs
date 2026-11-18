//! kill — HybridOS Process Signaller (Ring 3 ELF64).
//!
//! Uses the REAL `sys_kill` (62) system call to send signals to kernel
//! processes. Demonstrates signal 0 (null signal / existence check) on the
//! current process, init (PID 1) and the idle process (PID 0).

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
    println!("[kill] fatal: {:?}", info);
    sys_exit(1);
}

const SIG_NULL: i32 = 0;
const SIG_TERM: i32 = 15;
const SIG_KILL: i32 = 9;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let my_pid = sys_getpid();

    println!("");
    println!("============================================================");
    println!("                kill - HybridOS Process Signaller           ");
    println!("============================================================");
    println!("  current PID : {}", my_pid);
    println!("  real syscall: sys_kill (Linux ABI #62)");
    println!("");

    // Null signal (0): existence / permission check — does NOT terminate.
    println!("[signal 0: existence checks]");
    for pid in [0i64, 1i64, my_pid as i64] {
        let ret = unsafe { sys_kill(pid, SIG_NULL) };
        if ret == 0 {
            println!("  PID {} : exists (reachable)", pid);
        } else {
            println!("  PID {} : not reachable (ret={})", pid, ret);
        }
    }

    println!("");
    println!("[signal 15 (SIGTERM) on self - requesting termination]");
    let ret = unsafe { sys_kill(my_pid as i64, SIG_TERM) };
    println!("  sys_kill({}, SIGTERM) = {}", my_pid, ret);

    println!("");
    println!("[kill] real kernel sys_kill exercised. Exiting cleanly.");
    sys_sched_yield();
    sys_exit(0);
}
