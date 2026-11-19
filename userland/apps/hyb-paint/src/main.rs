//! hyb-paint: Editor Rapido de Anotacoes e Imagens (Ring 3 ELF64)
//! Linha de Bresenham, retangulo preenchido, desfoque box-blur 3x3 e cabecalho PNG.

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
    println!("[hyb-paint CRASH] {:?}", info);
    sys_exit(1);
}

const PW: usize = 40;
const PH: usize = 20;
type Canvas = [[u8; PW]; PH]; // 0 = branco, 1 = traco

fn limpar(canvas: &mut Canvas) {
    let mut y = 0;
    while y < PH {
        let mut x = 0;
        while x < PW {
            canvas[y][x] = 0;
            x += 1;
        }
        y += 1;
    }
}

fn desenhar_ponto(canvas: &mut Canvas, x: isize, y: isize) {
    if x >= 0 && x < PW as isize && y >= 0 && y < PH as isize {
        canvas[y as usize][x as usize] = 1;
    }
}

// Linha real (algoritmo de Bresenham)
fn desenhar_linha(canvas: &mut Canvas, x0: isize, y0: isize, x1: isize, y1: isize) {
    let mut dx = (x1 - x0).abs();
    let mut dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        desenhar_ponto(canvas, x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

// Retangulo preenchido real
fn preencher_retangulo(canvas: &mut Canvas, x0: isize, y0: isize, x1: isize, y1: isize) {
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            desenhar_ponto(canvas, x, y);
            x += 1;
        }
        y += 1;
    }
}

// Desfoque real (box blur 3x3)
fn aplicar_blur(canvas: &Canvas, saida: &mut Canvas) {
    let mut y = 0;
    while y < PH {
        let mut x = 0;
        while x < PW {
            let mut soma = 0u32;
            let mut n = 0u32;
            let mut yy = y as isize - 1;
            while yy <= y as isize + 1 {
                let mut xx = x as isize - 1;
                while xx <= x as isize + 1 {
                    if xx >= 0 && xx < PW as isize && yy >= 0 && yy < PH as isize {
                        soma += canvas[yy as usize][xx as usize] as u32;
                        n += 1;
                    }
                    xx += 1;
                }
                yy += 1;
            }
            saida[y][x] = if n > 0 { (soma / n) as u8 } else { 0 };
            x += 1;
        }
        y += 1;
    }
}

fn render(canvas: &Canvas) {
    let mut y = 0;
    while y < PH {
        let mut x = 0;
        while x < PW {
            print!("{}", if canvas[y][x] != 0 { '#' } else { '.' });
            x += 1;
        }
        print!("\n");
        y += 1;
    }
}

// Cabecalho PNG real (assinatura + bloco IHDR)
fn escrever_cabecalho_png(buf: &mut [u8], largura: u32, altura: u32) -> usize {
    let assinatura: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut w = 0usize;
    while w < 8 && w < buf.len() {
        buf[w] = assinatura[w];
        w += 1;
    }
    // IHDR: comprimento 13, 'IHDR', largura, altura, profundidade 8, tipo 2 (RGB)
    let mut p = w;
    if p + 17 <= buf.len() {
        buf[p] = 13; p += 1;
        buf[p] = 0; p += 1;
        buf[p] = 0; p += 1;
        buf[p] = 0; p += 1; // length = 13
        buf[p] = b'I'; p += 1;
        buf[p] = b'H'; p += 1;
        buf[p] = b'D'; p += 1;
        buf[p] = b'R'; p += 1;
        buf[p] = (largura >> 24) as u8; p += 1;
        buf[p] = (largura >> 16) as u8; p += 1;
        buf[p] = (largura >> 8) as u8; p += 1;
        buf[p] = largura as u8; p += 1;
        buf[p] = (altura >> 24) as u8; p += 1;
        buf[p] = (altura >> 16) as u8; p += 1;
        buf[p] = (altura >> 8) as u8; p += 1;
        buf[p] = altura as u8; p += 1;
        buf[p] = 8; p += 1;  // bit depth
        buf[p] = 2; p += 1;  // color type RGB
        buf[p] = 0; p += 1;
        buf[p] = 0; p += 1;
        buf[p] = 0; p += 1;
    }
    p - w
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("           hyb-paint - Editor de Anotacoes e Imagens       ");
    println!("========================================================");
    let mut canvas = [[0u8; PW]; PH];
    limpar(&mut canvas);
    desenhar_linha(&mut canvas, 2, 2, 37, 2);   // topo
    desenhar_linha(&mut canvas, 2, 2, 2, 17);   // esquerda
    desenhar_linha(&mut canvas, 37, 2, 37, 17); // direita
    desenhar_linha(&mut canvas, 2, 17, 37, 17); // baixo
    desenhar_linha(&mut canvas, 4, 4, 35, 14);  // diagonal
    preencher_retangulo(&mut canvas, 10, 8, 15, 11);
    println!("[hyb-paint] Desenho (linha de Bresenham + retangulo preenchido):");
    render(&canvas);
    let mut blur = [[0u8; PW]; PH];
    aplicar_blur(&canvas, &mut blur);
    println!("[hyb-paint] Apos desfoque box-blur 3x3:");
    render(&blur);
    let mut png = [0u8; 64];
    let tam = escrever_cabecalho_png(&mut png, PW as u32, PH as u32);
    println!("[hyb-paint] Cabecalho PNG gravado: {} bytes (assinatura + IHDR)", tam);
    println!("[hyb-paint] Editor finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
