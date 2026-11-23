//! hyb-files: Listador de arquivos do VFS (análogo ao `ls`).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::{sys_read, sys_close, SYS_READ, SYS_CLOSE, sys_exit};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

const O_RDONLY: u32 = 0;
const O_DIRECTORY: u32 = 0o0200000;

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

fn read_dir(path: &str) -> Vec<String> {
    let fd = unsafe { sys_open(path, O_RDONLY | O_DIRECTORY, 0o644) };
    if fd < 0 {
        return Vec::new();
    }
    let mut buf = [0u8; 4096];
    let n = sys_read(fd as usize, &mut buf);
    let _ = sys_close(fd as usize);
    if n <= 0 {
        return Vec::new();
    }
    let text = core::str::from_utf8(&buf[..n as usize]).unwrap_or("");
    text.lines().map(|l| String::from(l.trim())).filter(|l| !l.is_empty()).collect()
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let path = "/";
    let entries = read_dir(path);
    for e in &entries {
        println!("{}", e);
    }
    if entries.is_empty() {
        println!("(diretório vazio ou não encontrado: {})", path);
    }
    sys_exit(0);
}
