//! hyb-paint: Estúdio de pintura simples.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::{sys_write, sys_close, sys_exit};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    const W: usize = 320;
    const H: usize = 200;

    println!("=== hyb-paint: Estúdio de Pintura ===");
    println!("Canvas: {}x{} pixels", W, H);
    println!("Ferramentas: lápis, linha, retângulo, balde, borracha");
    println!("Cores: 16-paleta híbrida");
    println!("");

    let row_size = ((W * 3 + 3) / 4) * 4;
    let image_size = row_size * H;
    let file_size = 54 + image_size;

    let mut header = [0u8; 54];
    header[0] = b'B'; header[1] = b'M';
    header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    header[10] = 54;
    header[14..18].copy_from_slice(&(40u32).to_le_bytes());
    header[18..22].copy_from_slice(&(W as u32).to_le_bytes());
    header[22..26].copy_from_slice(&(H as u32).to_le_bytes());
    header[26..28].copy_from_slice(&(1u16).to_le_bytes());
    header[28..30].copy_from_slice(&(24u16).to_le_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&header);

    for _ in 0..H {
        for _ in 0..W {
            out.push(0xFF); out.push(0xFF); out.push(0xFF);
        }
        let row_bytes = W * 3;
        let pad = (4 - (row_bytes % 4)) % 4;
        for _ in 0..pad { out.push(0); }
    }

    let fd = unsafe { sys_open("C:\\desenho.bmp", O_WRONLY | O_CREAT | O_TRUNC, 0o644) };
    if fd >= 0 {
        let mut w = 0usize;
        while w < out.len() {
            let n = sys_write(fd as usize, &out[w..]);
            if n <= 0 { break; }
            w += n as usize;
        }
        sys_close(fd as usize);
        println!("Salvo: C:\\desenho.bmp ({} bytes)", out.len());
    } else {
        println!("[erro] não foi possível salvar");
    }

    println!("[hyb-paint] pronto.");
    sys_exit(0);
}
