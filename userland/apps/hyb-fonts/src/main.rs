//! hyb-fonts: Gerenciador e Renderizador Tipografico (Ring 3 ELF64)
//! Leitura de cabecalho TrueType (.ttf/.otf), rasterizacao de glifo ASCII e ligaduras.

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
    println!("[hyb-fonts CRASH] {:?}", info);
    sys_exit(1);
}

#[repr(C)]
struct TtfHeader {
    sfnt: u32,
    num_tables: u16,
    search_range: u16,
    entry_selector: u16,
    range_shift: u16,
}

// Le o cabecalho de uma fonte TrueType (offset table real)
fn parse_ttf_header(buf: &[u8]) -> Option<TtfHeader> {
    if buf.len() < 12 {
        return None;
    }
    let sfnt = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let num_tables = u16::from_be_bytes([buf[4], buf[5]]);
    let search_range = u16::from_be_bytes([buf[6], buf[7]]);
    let entry_selector = u16::from_be_bytes([buf[8], buf[9]]);
    let range_shift = u16::from_be_bytes([buf[10], buf[11]]);
    Some(TtfHeader { sfnt, num_tables, search_range, entry_selector, range_shift })
}

// Renderizacao de um glifo simples como bitmap ASCII (simulacao de rasterizacao)
fn renderizar_glifo_ascii(largura: usize, altura: usize, pixels: &[u8]) {
    let mut y = 0;
    while y < altura {
        let mut x = 0;
        while x < largura {
            let idx = y * largura + x;
            let c = if idx < pixels.len() && pixels[idx] > 128 { '#' } else { '.' };
            print!("{}", c);
            x += 1;
        }
        print!("\n");
        y += 1;
    }
}

// Detecta ligaduras de programacao em uma linha de texto
fn detectar_ligaduras(linha: &str) -> usize {
    let b = linha.as_bytes();
    let mut n = 0usize;
    let mut i = 0;
    while i + 1 < b.len() {
        if (b[i] == b'=' && b[i + 1] == b'>')
            || (b[i] == b'!' && b[i + 1] == b'=')
            || (b[i] == b':' && b[i + 1] == b':')
        {
            let par: [u8; 2] = [b[i], b[i + 1]];
            print!("ligadura na posicao {}: {}", i, core::str::from_utf8(&par).unwrap_or("??"));
            print!("\n");
            n += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    n
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-fonts - Gerenciador e Renderizador Tipografico  ");
    println!("========================================================");

    // Cabecalho TTF simulado (sfnt 'true' = 0x74727561, num_tables = 12)
    let ttf: &[u8] = b"\x74\x72\x75\x61\x00\x0C\x00\x10\x00\x04\x00\x08";
    if let Some(h) = parse_ttf_header(ttf) {
        println!("[hyb-fonts] sfnt = 0x{:08X}", h.sfnt);
        println!("[hyb-fonts] num_tables = {}", h.num_tables);
        println!("[hyb-fonts] search_range = {}", h.search_range);
        println!("[hyb-fonts] fonte TrueType valida");
    }

    println!("[hyb-fonts] Glifo simulado (letra 'H' 5x7):");
    let pixels: [u8; 35] = [
        0, 255, 0, 255, 0,
        0, 255, 0, 255, 0,
        0, 255, 255, 255, 0,
        0, 255, 0, 255, 0,
        0, 255, 0, 255, 0,
        0, 255, 0, 255, 0,
        0, 255, 0, 255, 0,
    ];
    renderizar_glifo_ascii(5, 7, &pixels);

    println!("[hyb-fonts] Detecao de ligaduras:");
    let linhas = ["x => y", "a !== b", "c::d", "normal texto"];
    let mut i = 0;
    while i < linhas.len() {
        let n = detectar_ligaduras(linhas[i]);
        println!("  '{}' -> {} ligadura(s)", linhas[i], n);
        i += 1;
    }
    println!("[hyb-fonts] Renderizador finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
