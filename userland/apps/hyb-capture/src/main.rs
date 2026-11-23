//! hyb-capture: Captura de tela.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    const FB_ADDR: usize = 0xFFFF_A000_0000_0000;
    const W: usize = 1280;
    const H: usize = 800;
    const PITCH: usize = 5120;

    println!("=== hyb-capture ===");
    println!("Framebuffer: {}x{} @ 0x{:X}", W, H, FB_ADDR);

    let mut header = [0u8; 54];
    header[0] = b'B'; header[1] = b'M';
    let file_size = 54 + W * H * 3;
    header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    header[10] = 54;
    header[14..18].copy_from_slice(&(40u32).to_le_bytes());
    header[18..22].copy_from_slice(&(W as u32).to_le_bytes());
    header[22..26].copy_from_slice(&(H as u32).to_le_bytes());
    header[26..28].copy_from_slice(&(1u16).to_le_bytes());
    header[28..30].copy_from_slice(&(24u16).to_le_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&header);

    for y in (0..H).rev() {
        for x in 0..W {
            let offset = y * PITCH + x * 4;
            let ptr = (FB_ADDR + offset) as *const u32;
            let pixel = unsafe { ptr.read_volatile() };
            let b = (pixel & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let r = ((pixel >> 16) & 0xFF) as u8;
            out.push(b); out.push(g); out.push(r);
        }
    }

    use libhybrid_sys::linux::{sys_open, sys_write, sys_close};
    const O_WRONLY: u32 = 1;
    const O_CREAT: u32 = 0x40;
    const O_TRUNC: u32 = 0x200;
    let fd = sys_open("/screenshot.bmp", O_WRONLY | O_CREAT | O_TRUNC, 0o644);
    if fd >= 0 {
        let mut w = 0usize;
        while w < out.len() {
            let n = libhybrid_sys::linux::sys_write(fd as usize, &out[w..]);
            if n <= 0 { break; }
            w += n as usize;
        }
        sys_close(fd as usize);
        println!("Salvo: /screenshot.bmp ({} bytes)", out.len());
    } else {
        println!("[erro] não foi possível salvar no VFS");
    }

    sys_exit(0);
}
