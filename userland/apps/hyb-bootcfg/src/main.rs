//! hyb-bootcfg — HybridOS Boot Configuration Reader (Ring 3 ELF64).
//!
//! Reads the boot loader / system configuration through the VFS
//! (/boot/config, /etc/bootcfg, /etc/msconfig.conf). Reports boot
//! parameters and boot device. Degrades gracefully without a config file.

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
    println!("[hyb-bootcfg] fatal: {:?}", info);
    sys_exit(1);
}

const BOOT_PATHS: [&str; 4] = ["/boot/config", "/etc/bootcfg", "/etc/msconfig.conf", "/boot/grub.cfg"];

fn read_file(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0, 0) };
    if fd < 0 { return false; }
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
    println!("\n[hyb-bootcfg] {} bytes read from {}", total, path);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-bootcfg - HybridOS Boot Configuration Reader      ");
    println!("============================================================");

    println!("[boot configuration]");
    println!("  boot loader : Limine (x86_64 Long Mode)");
    println!("  kernel      : hybridos-kernel (bare-metal, #![no_std])");
    println!("  personality : Dual-ABI (Windows NT + Linux POSIX)");
    println!("  memory map  : consumed from Limine memmap");

    let mut found = false;
    for path in BOOT_PATHS.iter() {
        if read_file(path) {
            found = true;
            break;
        }
    }
    if !found {
        println!("  no boot config file exposed via VFS");
        println!("  (checked /boot/config, /etc/bootcfg, /etc/msconfig.conf, /boot/grub.cfg)");
    }

    println!("");
    println!("[hyb-bootcfg] boot configuration read.");
    sys_sched_yield();
    sys_exit(0);
}
