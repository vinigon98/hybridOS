//! hyb-zoom: Lupa de Compositor e Color Picker (Ring 3 ELF64)
//! Zoom suave em nível de GPU (Super + Scroll) e conta-gotas (Super + Shift + C) HEX/RGB.

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
    println!("[hyb-zoom CRASH] {:?}", info);
    sys_exit(1);
}

const ZW: usize = 16;
const ZH: usize = 12;
type Tela = [[u32; ZW]; ZH]; // ARGB32

fn nibble(v: u8) -> u8 {
    if v < 10 {
        b'0' + v
    } else {
        b'A' + (v - 10)
    }
}

// Amostra um pixel e retorna (R, G, B)
fn sample_pixel(tela: &Tela, x: usize, y: usize) -> Option<(u8, u8, u8)> {
    if x < ZW && y < ZH {
        let v = tela[y][x];
        Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
    } else {
        None
    }
}

// Imprime o codigo HEX de uma cor RGB
fn imprimir_hex(r: u8, g: u8, b: u8) {
    let h: [u8; 7] = [b'#', nibble(r >> 4), nibble(r & 0xF), nibble(g >> 4), nibble(g & 0xF), nibble(b >> 4), nibble(b & 0xF)];
    println!("{}", core::str::from_utf8(&h).unwrap_or("#??????"));
}

// Zoom real: amplia um retangulo da tela por um fator inteiro (replicacao de pixels)
fn zoom_regiao(tela: &Tela, x0: usize, y0: usize, x1: usize, y1: usize, fator: usize, saida: &mut Tela) {
    let mut y = 0;
    while y < ZH {
        let mut x = 0;
        while x < ZW {
            saida[y][x] = 0;
            x += 1;
        }
        y += 1;
    }
    let mut dy = 0;
    let mut sy = y0;
    while sy < y1.min(ZH) && dy < ZH {
        let mut dx = 0;
        let mut sx = x0;
        while sx < x1.min(ZW) && dx < ZW {
            let cor = tela[sy][sx];
            let mut rep = 0;
            while rep < fator && dx + rep < ZW {
                saida[dy][dx + rep] = cor;
                rep += 1;
            }
            dx += fator;
            sx += 1;
        }
        // replica a linha produzida para as linhas seguintes do bloco de zoom
        let mut rep2 = 1;
        while rep2 < fator && dy + rep2 < ZH {
            let mut dx2 = 0;
            while dx2 < ZW {
                saida[dy + rep2][dx2] = saida[dy][dx2];
                dx2 += 1;
            }
            rep2 += 1;
        }
        dy += fator;
        sy += 1;
    }
}

fn render_hex(tela: &Tela) {
    let mut y = 0;
    while y < ZH {
        let mut x = 0;
        while x < ZW {
            let v = tela[y][x];
            print!("{:02X}{:02X}{:02X} ", (v >> 16) as u8, (v >> 8) as u8, v as u8);
            x += 1;
        }
        print!("\n");
        y += 1;
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("            hyb-zoom - Lupa de Compositor e Color Picker       ");
    println!("========================================================");
    let mut tela = [[0u32; ZW]; ZH];
    // preenche tela com gradiente simulado
    let mut y = 0;
    while y < ZH {
        let mut x = 0;
        while x < ZW {
            tela[y][x] = ((x * 16) as u32) << 16 | ((y * 20) as u32) << 8 | ((x + y) * 4) as u32;
            x += 1;
        }
        y += 1;
    }
    println!("[hyb-zoom] Color picker: pixel (4,5) = ");
    if let Some((r, g, b)) = sample_pixel(&tela, 4, 5) {
        println!("  RGB = ({}, {}, {})", r, g, b);
        print!("  HEX = ");
        imprimir_hex(r, g, b);
    }
    println!("[hyb-zoom] Zoom (Super + Scroll) da regiao (2,2)-(10,8) fator 2:");
    let mut zoom = [[0u32; ZW]; ZH];
    zoom_regiao(&tela, 2, 2, 10, 8, 2, &mut zoom);
    render_hex(&zoom);
    println!("[hyb-zoom] Lupa finalizada.");
    sys_sched_yield();
    sys_exit(0)
}
