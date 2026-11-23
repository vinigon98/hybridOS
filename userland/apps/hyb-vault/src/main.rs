//! hyb-vault: Cofre de senhas.

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

const VAULT_PATH: &str = "/etc/vault";
const O_RDONLY: u32 = 0;

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

fn read_vault() -> Option<Vec<u8>> {
    let fd = unsafe { sys_open(VAULT_PATH, O_RDONLY, 0o644) };
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
    println!("=== hyb-vault: Cofre de Senhas ===");
    println!("Armazenamento: /etc/vault (VFS)");
    println!("");
    match read_vault() {
        Some(contents) => {
            let text = core::str::from_utf8(&contents).unwrap_or("");
            if text.is_empty() {
                println!("Cofre vazio.");
            } else {
                println!("Entradas armazenadas:");
                println!("{}", text);
            }
        }
        None => {
            println!("Cofre não existe ainda.");
        }
    }
    sys_exit(0);
}
