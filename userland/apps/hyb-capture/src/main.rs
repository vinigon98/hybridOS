//! hyb-capture: Motor de Captura e OCR Local (Ring 3 ELF64)
//! Captura de regiao, gravacao MP4/MKV simulada (cabecalho ftyp real) e OCR offline de imagens.

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
    println!("[hyb-capture CRASH] {:?}", info);
    sys_exit(1);
}

const W: usize = 80;
const H: usize = 24;
type Frame = [[u8; W]; H];

// Captura real de uma regiao do buffer do compositor (simulada)
fn capturar_regiao(frame: &Frame, x0: usize, y0: usize, x1: usize, y1: usize, out: &mut Frame) {
    let mut y = 0;
    while y < H {
        let mut x = 0;
        while x < W {
            out[y][x] = 0;
            x += 1;
        }
        y += 1;
    }
    let mut dy = 0;
    let mut y = y0;
    while y < y1.min(H) {
        let mut dx = 0;
        let mut x = x0;
        while x < x1.min(W) {
            out[dy][dx] = frame[y][x];
            dx += 1;
            x += 1;
        }
        dy += 1;
        y += 1;
    }
}

// OCR local simplificado: varre o frame ASCII e extrai sequencias alfanumericas
fn ocr_extrair_texto(frame: &Frame) -> usize {
    let mut total = 0usize;
    let mut y = 0;
    while y < H {
        let mut x = 0;
        while x < W {
            let c = frame[y][x];
            if (c >= b'A' && c <= b'Z') || (c >= b'a' && c <= b'z') || (c >= b'0' && c <= b'9') {
                print!("{}", c as char);
                total += 1;
            } else if c != 0 {
                print!(" ");
            }
            x += 1;
        }
        print!("\n");
        y += 1;
    }
    total
}

// Cabecalho MP4/MKV simulado (ftyp box real do ISO BMFF)
fn escrever_cabecalho_mp4(buf: &mut [u8]) -> usize {
    let mut w = 0usize;
    if w + 8 <= buf.len() {
        buf[w] = 0x00; w += 1;
        buf[w] = 0x00; w += 1;
        buf[w] = 0x00; w += 1;
        buf[w] = 0x20; w += 1; // size = 32
        buf[w] = b'f'; w += 1;
        buf[w] = b't'; w += 1;
        buf[w] = b'y'; w += 1;
        buf[w] = b'p'; w += 1;
        buf[w] = b'i'; w += 1;
        buf[w] = b's'; w += 1;
        buf[w] = b'o'; w += 1;
        buf[w] = b'm'; w += 1;
        buf[w] = 0; w += 1; // minor version
        buf[w] = 0; w += 1;
        buf[w] = 0; w += 1;
        buf[w] = 0; w += 1;
    }
    w
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("       hyb-capture - Motor de Captura e OCR Local        ");
    println!("========================================================");

    let mut tela = [[0u8; W]; H];
    let mensagem = b"HybridOS OCR local ativo";
    let mut y = 10usize;
    let mut x = 2usize;
    while x < 2usize + mensagem.len() && x < W {
        tela[y][x] = mensagem[x - 2];
        x += 1;
    }

    let mut recorte = [[0u8; W]; H];
    capturar_regiao(&tela, 0, 8, W, 14, &mut recorte);
    println!("[hyb-capture] Regiao capturada (0,8)-(80,14):");
    let n = ocr_extrair_texto(&recorte);
    println!("[hyb-capture] OCR extraiu {} caracteres alfanumericos", n);

    let mut cab = [0u8; 64];
    let tam = escrever_cabecalho_mp4(&mut cab);
    println!("[hyb-capture] Cabecalho MP4 (ftyp) gravado: {} bytes", tam);
    println!("[hyb-capture] Capturador finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
