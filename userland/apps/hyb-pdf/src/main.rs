//! hyb-pdf: Visualizador de documentos PDF.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::{sys_read, sys_close, SYS_READ, SYS_CLOSE, sys_exit};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

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
    let path = "/documento.pdf";
    println!("=== hyb-pdf: Visualizador PDF ===");
    println!("Arquivo: {}", path);
    println!("");

    match read_file(path) {
        Some(contents) => {
            if contents.len() >= 5 && &contents[0..5] == b"%PDF-" {
                println!("Formato: PDF válido");
                println!("Tamanho: {} bytes", contents.len());
                let text = core::str::from_utf8(&contents).unwrap_or("");
                println!("Conteúdo extraído:");
                println!("----------------------------------------");
                let mut in_text = false;
                for ch in text.chars() {
                    if ch == '(' { in_text = true; }
                    else if ch == ')' { in_text = false; print!(" "); }
                    else if in_text && ch.is_ascii_graphic() { print!("{}", ch); }
                }
                println!("");
                println!("----------------------------------------");
            } else {
                println!("[erro] arquivo não é um PDF válido");
            }
        }
        None => {
            println!("[erro] não foi possível ler: {}", path);
        }
    }
    sys_exit(0);
}
