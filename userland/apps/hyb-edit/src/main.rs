//! hyb-edit: Editor de texto simples.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::{sys_read, sys_close, SYS_READ, SYS_CLOSE, sys_write, sys_exit};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

const O_RDONLY: u32 = 0;
const O_WRONLY: u32 = 1;
const O_CREAT: u32 = 0x40;
const O_TRUNC: u32 = 0x200;

#[inline(always)]
unsafe fn syscall3(nr: u64, a1: u64, a2: u64, a3: u64) -> isize {
    let ret: isize;
    core::arch::asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, in("rsi") a2, in("rdx") a3, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}

#[inline(always)]
unsafe fn syscall1(nr: u64, a1: u64) -> isize {
    let ret: isize;
    core::arch::asm!("syscall", inlateout("rax") nr => ret, in("rdi") a1, out("rcx") _, out("r11") _, options(nostack, preserves_flags));
    ret
}

unsafe fn sys_open(path: &str, flags: u32, mode: u32) -> isize {
    syscall3(2, path.as_ptr() as u64, flags as u64, mode as u64)
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = unsafe { sys_open(path, O_RDONLY, 0o644) };
    if fd < 0 { return None; }
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = sys_read(fd as usize, &mut buf);
        if n <= 0 { break; }
        out.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd as usize);
    Some(out)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let path = "/readme.txt";
    println!("=== hyb-edit: {} ===", path);
    match read_file(path) {
        Some(contents) => {
            let text = core::str::from_utf8(&contents).unwrap_or("");
            print!("{}", text);
            println!("\n=== fim do arquivo ===");
        }
        None => {
            println!("[hyb-edit] não foi possível ler: {}", path);
        }
    }
    sys_exit(0);
}
