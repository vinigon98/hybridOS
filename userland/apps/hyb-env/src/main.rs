//! hyb-env — HybridOS Environment Viewer (Ring 3 ELF64).
//!
//! Displays the current process environment: PID, execution personality and
//! kernel-visible environment. Attempts to read /etc/environment through the
//! VFS. Real syscalls: sys_getpid (39), sys_open/sys_read.

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
    println!("[hyb-env] fatal: {:?}", info);
    sys_exit(1);
}

fn read_file(path: &str) -> Option<usize> {
    let fd = unsafe { sys_open(path, 0, 0) };
    if fd < 0 { return None; }
    let mut buf = [0u8; 1024];
    let mut total = 0usize;
    loop {
        let ret = unsafe { sys_read(fd as usize, &mut buf) };
        if ret <= 0 { break; }
        let n = ret as usize;
        let _ = print!("{}", core::str::from_utf8(&buf[..n]).unwrap_or("<binary>"));
        total += n;
    }
    let _ = sys_close(fd as usize);
    Some(total)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let pid = sys_getpid();

    println!("");
    println!("============================================================");
    println!("              hyb-env - HybridOS Environment Viewer          ");
    println!("============================================================");

    println!("[process environment]");
    println!("  PID          : {}", pid);
    println!("  personality  : Dual-ABI (Linux System V AMD64 + Windows NT x64)");
    println!("  syscall ABI  : Linux POSIX (SysV AMD64) via libhybrid_sys");
    println!("  GS base      : TEB / Per-CPU data (ring 3)");
    println!("  KUSER_SHARED_DATA: 0x7FFE0000");

    println!("");
    println!("[system environment file /etc/environment]");
    match read_file("/etc/environment") {
        Some(bytes) => println!("\n[hyb-env] {} bytes read from /etc/environment", bytes),
        None => {
            println!("  /etc/environment not present on this build");
            println!("  (no user/system environment variables persisted yet)");
        }
    }

    println!("");
    println!("[hyb-env] environment read complete.");
    sys_sched_yield();
    sys_exit(0);
}
