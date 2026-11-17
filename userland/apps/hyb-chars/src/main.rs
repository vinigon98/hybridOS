//! hyb-chars: Inspetor Unicode e Simbolos Tecnicos (Ring 3 ELF64)
//! Tabela ASCII, categorias Unicode, entidades HTML e simbolos matematicos.

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
    println!("[hyb-chars CRASH] {:?}", info);
    sys_exit(1);
}

fn categoria(c: u32) -> &'static str {
    match c {
        0..=0x1F => "Controle",
        0x20..=0x7E => "Imprimivel ASCII",
        0x7F => "Del",
        0x80..=0xFF => "Latin-1 Suplemento",
        0x2500..=0x257F => "Geometricos",
        0x2190..=0x21FF => "Setas",
        0x2200..=0x22FF => "Matematicos",
        0x0370..=0x03FF => "Grego",
        0x0400..=0x04FF => "Cirilico",
        _ => "Outro Unicode",
    }
}

fn html_entidade(c: u32) -> Option<&'static str> {
    match c {
        0x2264 => Some("&le;"),
        0x2265 => Some("&ge;"),
        0x00D7 => Some("&times;"),
        0x00F7 => Some("&divide;"),
        0x221E => Some("&infin;"),
        0x03C0 => Some("&pi;"),
        _ => None,
    }
}

fn mostrar_codigo(c: u32, glyph: char) {
    print!("U+{:04X}  ", c);
    print!("{}  ", glyph);
    print!("{}", categoria(c));
    if let Some(e) = html_entidade(c) {
        print!("  {}", e);
    }
    println!();
}

fn tabela_ascii() {
    println!("[hyb-chars] Tabela ASCII (0..128):");
    let mut c = 0u32;
    while c < 128 {
        let glyph = if c >= 32 && c < 127 { c as u8 as char } else { '.' };
        print!("{:3} '{}'  ", c, glyph);
        if c % 8 == 7 {
            println!();
        }
        c += 1;
    }
    println!();
}

fn busca_simbolo() {
    println!("[hyb-chars] Simbolos tecnicos relevantes:");
    let simbolos: [(u32, char, &str); 8] = [
        (0x2264, '\u{2264}', "Menor ou igual"),
        (0x2265, '\u{2265}', "Maior ou igual"),
        (0x221E, '\u{221E}', "Infinito"),
        (0x03C0, '\u{03C0}', "Pi"),
        (0x00D7, '\u{00D7}', "Multiplicacao"),
        (0x2192, '\u{2192}', "Seta para direita"),
        (0x25A0, '\u{25A0}', "Quadrado preenchido"),
        (0x03A9, '\u{03A9}', "Ohms / Omega"),
    ];
    let mut i = 0;
    while i < simbolos.len() {
        let (cp, g, nome) = simbolos[i];
        print!("U+{:04X} {}  ", cp, g);
        println!("{}", nome);
        i += 1;
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-chars - Inspetor Unicode e Simbolos           ");
    println!("========================================================");
    tabela_ascii();
    println!();
    busca_simbolo();
    println!();
    mostrar_codigo(0x03C0, '\u{03C0}');
    mostrar_codigo(0x221E, '\u{221E}');
    mostrar_codigo(0x00D7, '\u{00D7}');
    if let Some(e) = html_entidade(0x2264) {
        println!("[hyb-chars] Entidade HTML de U+2264: {}", e);
    }
    println!("[hyb-chars] Inspetor finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
