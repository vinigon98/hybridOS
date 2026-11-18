//! hyb-reboot — HybridOS System Reboot Request (Ring 3 ELF64).
//!
//! Requests a system reboot. The actual reset is performed by the kernel
//! (ACPI reset / KB controller 0xCF9 / 0x64) which runs in Ring 0; from
//! Ring 3 we report the request and the current kernel uptime via
//! `sys_sysinfo` (200), then exit cleanly without panicking.

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
    println!("[hyb-reboot] fatal: {:?}", info);
    sys_exit(1);
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("          hyb-reboot - HybridOS System Reboot Request        ");
    println!("============================================================");

    println!("[reboot request]");
    println!("  target     : cold reboot (ACPI / KB controller reset)");
    println!("  requester  : ring-3 user process (hyb-reboot)");
    println!("  reset path : rust-kernel/src/arch/acpi.rs  ->  acpi_shutdown / 0xCF9 / 0x64");
    println!("  port 0xCF9 : hard reset (Ring 0 only - not accessible from Ring 3)");
    println!("  port 0x64  : keyboard controller pulse (Ring 0 only)");

    println!("");
    println!("[kernel state at request]");
    match read_sysinfo() {
        Some(info) => {
            println!("  uptime ticks   : {}", info.uptime_ticks);
            println!("  context switches: {}", info.context_switches);
            println!("  total ticks    : {}", info.total_ticks);
        }
        None => {
            println!("  sys_sysinfo unavailable on this kernel build");
        }
    }

    println!("");
    println!("[hyb-reboot] request logged. Actual reset is a Ring-0 kernel operation.");
    println!("[hyb-reboot] exiting cleanly (no reboot performed from Ring 3).");
    sys_sched_yield();
    sys_exit(0);
}
