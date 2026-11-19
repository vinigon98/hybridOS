//! hyb-pdf: Motor Nativo de Leitura e Impressao PDF (Ring 3 ELF64)
//! Leitor ultraleve de PDF: cabecalho %PDF-, contagem de objetos, extrator de texto e impressora virtual.

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
    println!("[hyb-pdf CRASH] {:?}", info);
    sys_exit(1);
}

const MAX_OBJ: usize = 32;

// Verifica cabecalho PDF real (%PDF-1.x)
fn pdf_valido(dados: &[u8]) -> bool {
    dados.len() >= 5
        && dados[0] == b'%'
        && dados[1] == b'P'
        && dados[2] == b'D'
        && dados[3] == b'F'
        && dados[4] == b'-'
}

// Conta objetos reais (marcadores 'obj'/'endobj')
fn contar_objetos(dados: &[u8]) -> usize {
    let mut n = 0usize;
    let mut i = 0;
    while i + 3 <= dados.len() {
        if dados[i] == b'o' && dados[i + 1] == b'b' && dados[i + 2] == b'j' {
            n += 1;
        }
        i += 1;
    }
    n
}

// Extrai texto real de strings entre parenteses (conteudo do PDF)
fn extrair_texto(dados: &[u8], saida: &mut [u8]) -> usize {
    let mut w = 0usize;
    let mut i = 0;
    while i < dados.len() {
        if dados[i] == b'(' {
            i += 1;
            while i < dados.len() && dados[i] != b')' && w < saida.len().saturating_sub(1) {
                saida[w] = dados[i];
                w += 1;
                i += 1;
            }
            if w < saida.len() {
                saida[w] = b' ';
                w += 1;
            }
        }
        i += 1;
    }
    w
}

// Gera um PDF minimo valido (cabecalho, objeto catalogo, stream de texto, trailer)
fn gerar_pdf_minimo(buf: &mut [u8], texto: &str) -> usize {
    let cab: &[u8] = b"%PDF-1.4\n";
    let mut w = 0usize;
    while w < cab.len() && w < buf.len() {
        buf[w] = cab[w];
        w += 1;
    }
    // objeto 1: catalogo
    let obj: &[u8] = b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
    let mut o = 0;
    while o < obj.len() && w < buf.len() {
        buf[w] = obj[o];
        w += 1;
        o += 1;
    }
    // objeto 2: pagina com stream de texto entre parenteses
    let tb = texto.as_bytes();
    if w + 40 < buf.len() {
        let s: &[u8] = b"2 0 obj\n<< /Length ";
        let mut q = 0;
        while q < s.len() && w < buf.len() {
            buf[w] = s[q];
            w += 1;
            q += 1;
        }
        let len = tb.len();
        let d1 = (len / 100) as u8 + b'0';
        let d2 = ((len % 100) / 10) as u8 + b'0';
        let d3 = (len % 10) as u8 + b'0';
        if w < buf.len() { buf[w] = d1; w += 1; }
        if w < buf.len() { buf[w] = d2; w += 1; }
        if w < buf.len() { buf[w] = d3; w += 1; }
        let s2: &[u8] = b">>\nstream\n(";
        q = 0;
        while q < s2.len() && w < buf.len() {
            buf[w] = s2[q];
            w += 1;
            q += 1;
        }
        let mut t = 0;
        while t < tb.len() && w < buf.len() {
            buf[w] = tb[t];
            w += 1;
            t += 1;
        }
        let s3: &[u8] = b")\nendstream\nendobj\n";
        q = 0;
        while q < s3.len() && w < buf.len() {
            buf[w] = s3[q];
            w += 1;
            q += 1;
        }
    }
    // trailer
    let tr: &[u8] = b"trailer\n<< /Root 1 0 R >>\n%%EOF\n";
    let mut q = 0;
    while q < tr.len() && w < buf.len() {
        buf[w] = tr[q];
        w += 1;
        q += 1;
    }
    w
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("           hyb-pdf - Motor Nativo de Leitura PDF           ");
    println!("========================================================");
    let amostra: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog>>endobj\ntexto (Ola HybridOS PDF)\n";
    println!("[hyb-pdf] Cabecalho PDF valido? {}", pdf_valido(amostra));
    println!("[hyb-pdf] Objetos encontrados: {}", contar_objetos(amostra));
    let mut txt = [0u8; 64];
    let n = extrair_texto(amostra, &mut txt);
    println!("[hyb-pdf] Texto extraido: {}", core::str::from_utf8(&txt[..n]).unwrap_or(""));
    let mut pdf = [0u8; 200];
    let tam = gerar_pdf_minimo(&mut pdf, "Relatorio HybridOS - impressao virtual");
    println!("[hyb-pdf] PDF minimo gerado: {} bytes (Print to PDF)", tam);
    println!("[hyb-pdf] Leitor finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
