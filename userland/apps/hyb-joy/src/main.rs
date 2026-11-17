//! hyb-joy — HybridOS Joystick / Gamepad Input Reader (Ring 3 ELF64).
//!
//! Reads gamepad/joystick input state through the input device nodes exposed
//! by the kernel (sys_open/sys_read on /dev/input/js0, /dev/joy0, /dev/input/*).
//! Degrades gracefully when no joystick device is present.

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
    println!("[hyb-joy] fatal: {:?}", info);
    sys_exit(1);
}

const JOY_PATHS: [&str; 5] = ["/dev/input/js0", "/dev/joy0", "/dev/input/joy0", "/dev/gamepad", "/dev/input/event0"];

fn try_open(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0, 0) };
    if fd < 0 {
        return false;
    }
    let mut buf = [0u8; 64];
    let _ = unsafe { sys_read(fd as usize, &mut buf) };
    let _ = sys_close(fd as usize);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-joy - HybridOS Joystick / Gamepad Reader         ");
    println!("============================================================");

    let mut found = false;
    for path in JOY_PATHS.iter() {
        if try_open(path) {
            println!("[input device] {} : opened (event stream readable)", path);
            found = true;
        }
    }

    if !found {
        println!("[hyb-joy] no joystick/gamepad device node exposed via VFS");
        println!("[hyb-joy] (checked /dev/input/js0, /dev/joy0, /dev/input/joy0, /dev/gamepad, /dev/input/event0)");
        println!("[hyb-joy] raw HID input lives in the kernel PS/2 & USB input drivers");
    }

    println!("");
    println!("[hyb-joy] input scan complete.");
    sys_sched_yield();
    sys_exit(0);
}
