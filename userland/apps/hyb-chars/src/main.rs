//! hyb-chars: Mapa de caracteres ASCII.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::format;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-chars: Mapa ASCII ===");
    println!("Dec  Hex  Char  |  Dec  Hex  Char  |  Dec  Hex  Char");
    println!("------------------------------------------------");
    for row in 0..32 {
        let mut line = String::new();
        for col in 0..3 {
            let code = row + col * 32;
            let ch = if code >= 32 && code <= 126 {
                code as u8 as char
            } else {
                '.'
            };
            line.push_str(&format!("{:>3}  {:>2X}   '{}'    ", code, code, ch));
        }
        println!("{}", line.trim_end());
    }
    sys_exit(0);
}
