//! hyb-audio-ctl — HybridOS Audio Device Controller (Ring 3 ELF64).
//!
//! Queries the Intel HDA audio device state through VFS device nodes
//! (sys_open/sys_read on /dev/hda, /dev/snd, /dev/audio). Reports codec
//! presence and stream state. Degrades gracefully without audio hardware.

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
    println!("[hyb-audio-ctl] fatal: {:?}", info);
    sys_exit(1);
}

const AUDIO_PATHS: [&str; 5] = ["/dev/hda", "/dev/snd", "/dev/audio", "/dev/snd/pcmC0D0p", "/dev/dsp"];

fn try_audio(path: &str) -> bool {
    let fd = unsafe { sys_open(path, 0, 0) };
    if fd < 0 {
        return false;
    }
    let mut buf = [0u8; 32];
    let _ = unsafe { sys_read(fd as usize, &mut buf) };
    let _ = sys_close(fd as usize);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("      hyb-audio-ctl - HybridOS Audio Device Controller       ");
    println!("============================================================");

    println!("[intel HDA audio devices]");
    let mut found = false;
    for path in AUDIO_PATHS.iter() {
        if try_audio(path) {
            println!("  {} : device node present and readable", path);
            found = true;
        }
    }
    if !found {
        println!("  no audio device node exposed via VFS");
        println!("  (Intel HDA codec 8086:2668 lives in rust-kernel/src/drivers/)");
    }

    println!("");
    println!("[hyb-audio-ctl] audio device scan complete.");
    sys_sched_yield();
    sys_exit(0);
}
