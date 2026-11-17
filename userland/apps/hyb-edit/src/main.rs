//! hyb-edit: Editor de Texto Nativo Ultrarrápido (Ring 3 ELF64)
//! Leitura por blocos (estilo memory-mapped), contagem de linhas, conversao CRLF<->LF e busca de texto.

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
    println!("[hyb-edit CRASH] {:?}", info);
    sys_exit(1);
}

const BUF: usize = 2048;

// Leitura simulada de arquivo grande (memory-mapped style): le em blocos e estatisticas reais
fn scan_file(path: &str) -> (usize, usize, usize) {
    let mut buf = [0u8; BUF];
    let fd = sys_open(path, 0, 0);
    if fd < 0 {
        return (0, 0, 0);
    }
    let mut bytes = 0usize;
    let mut lines = 0usize;
    let mut crlf = 0usize;
    loop {
        let n = sys_read(fd as usize, &mut buf);
        if n <= 0 {
            break;
        }
        let slice = &buf[..n as usize];
        bytes += slice.len();
        let mut i = 0;
        while i < slice.len() {
            if slice[i] == b'\n' {
                lines += 1;
                if i > 0 && slice[i - 1] == b'\r' {
                    crlf += 1;
                }
            }
            i += 1;
        }
    }
    sys_close(fd as usize);
    (bytes, lines, crlf)
}

// Conta linhas de um buffer ja mapeado (memoria)
fn count_lines(buf: &[u8]) -> usize {
    let mut n = 0usize;
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == b'\n' {
            n += 1;
        }
        i += 1;
    }
    n
}

// Converte CRLF -> LF in-place, retorna quantidade de CR removidos
fn crlf_to_lf(buf: &mut [u8], len: usize) -> usize {
    let mut removed = 0usize;
    let mut w = 0usize;
    let mut r = 0usize;
    let lim = len.min(buf.len());
    while r < lim {
        if buf[r] == b'\r' && r + 1 < lim && buf[r + 1] == b'\n' {
            removed += 1;
            buf[w] = b'\n';
            w += 1;
            r += 2;
        } else {
            buf[w] = buf[r];
            w += 1;
            r += 1;
        }
    }
    removed
}

// Converte LF -> CRLF (saida em novo buffer), retorna tamanho gravado
fn lf_to_crlf(src: &[u8], dst: &mut [u8]) -> usize {
    let mut w = 0usize;
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\n' && w + 1 < dst.len() {
            dst[w] = b'\r';
            w += 1;
            dst[w] = b'\n';
            w += 1;
        } else if w < dst.len() {
            dst[w] = src[i];
            w += 1;
        }
        i += 1;
    }
    w
}

// Busca real de texto (substring) com retorno da primeira posicao
fn find_text(haystack: &[u8], needle: &[u8]) -> isize {
    if needle.is_empty() {
        return 0;
    }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        let mut m = true;
        let mut j = 0;
        while j < needle.len() {
            if haystack[i + j] != needle[j] {
                m = false;
                break;
            }
            j += 1;
        }
        if m {
            return i as isize;
        }
        i += 1;
    }
    -1
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-edit - Editor de Texto Nativo Ultrarrápido   ");
    println!("========================================================");
    let (bytes, lines, crlf) = scan_file("/etc/hybridos.toml");
    println!("[hyb-edit] /etc/hybridos.toml: {} bytes, {} linhas, {} CRLF", bytes, lines, crlf);

    let texto = b"HybridOS edit linha 1\nHybridOS edit linha 2\nHybridOS edit linha 3\n";
    println!("[hyb-edit] linhas no buffer: {}", count_lines(texto));

    let mut buf = [0u8; 256];
    let len = texto.len().min(buf.len());
    buf[..len].copy_from_slice(&texto[..len]);
    let removidos = crlf_to_lf(&mut buf, len);
    println!("[hyb-edit] CRLF->LF removeu {} retornos de carro", removidos);

    let mut saida = [0u8; 512];
    let n = lf_to_crlf(texto, &mut saida);
    println!("[hyb-edit] LF->CRLF gerou {} bytes", n);

    let pos = find_text(texto, b"edit");
    println!("[hyb-edit] 'edit' encontrado na posicao {}", pos);
    println!("[hyb-edit] Sessao de editor encerrada.");
    sys_sched_yield();
    sys_exit(0)
}
