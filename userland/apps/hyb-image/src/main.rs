//! hyb-image: Ferramenta de Imagem (ELF64)
//! Le e exibe informacoes de arquivos de imagem via VFS do kernel.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-image CRASH] {:?}", info);
    sys_exit(1);
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let mut buf = [0u8; 4096];
    let mut contents = Vec::new();
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd);
    Some(contents)
}

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

fn inspect_image(path: &str) {
    println!("  Imagem: {}", path);
    if !probe_path(path) {
        println!("    [--] Nao acessível via VFS");
        return;
    }
    if let Some(content) = read_file(path) {
        if content.is_empty() {
            println!("    [OK] Aberto via VFS, conteudo vazio");
            return;
        }
        println!("    [OK] Tamanho: {} bytes", content.len());
        // Detectar assinatura (magic number)
        let sig = if content.len() >= 4 { &content[0..4] } else { &content[..] };
        let sig_str: String = sig.iter().map(|b| if *b >= 32 && *b < 127 { *b as char } else { '.' }).collect();
        println!("    Assinatura (primeiros 4 bytes): {:02X}{:02X}{:02X}{:02X} ({})",
                 sig[0], sig[1], sig[2], sig[3], sig_str);
        if sig == b"\x89PNG" {
            println!("    Tipo detectado: PNG");
        } else if sig == b"\xFF\xD8\xFF" {
            println!("    Tipo detectado: JPEG");
        } else if sig == b"BM" {
            println!("    Tipo detectado: BMP");
        } else if sig == b"\x7FELF" {
            println!("    Tipo detectado: ELF64");
        } else if sig == b"MZ" {
            println!("    Tipo detectado: PE/EXE (Windows NT)");
        } else {
            println!("    Tipo: binario generico");
        }
    } else {
        println!("    [FALHA] Nao foi possivel ler conteudo via VFS");
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Ferramenta de Imagem (hyb-image)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Visualizador de imagens do sistema", pid);

    // Imagens e arquivos binarios visiveis pelo VFS
    let images = [
        "/c/Windows/System32/ntoskrnl.ini",
        "/etc/os-release",
        "/dev/zero",
        "/dev/null",
    ];

    println!();
    println!("--- Analise de arquivos de imagem/binario via VFS ---");
    for img in images.iter() {
        inspect_image(img);
        println!();
    }

    // Gravar uma imagem de teste no VFS
    println!("--- Gravando imagem de teste no VFS ---");
    let test_png: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG signature
    let fd = match sys_open("/tmp/test_image.png", 0, 0) {
        r if r >= 0 => r as usize,
        _ => 0,
    };
    if fd > 0 {
        let _ = sys_write(fd, &test_png);
        let _ = sys_close(fd);
        println!("  [OK] /tmp/test_image.png gravado (assinatura PNG)");
    } else {
        println!("  [FALHA] Nao foi possivel gravar imagem de teste");
    }

    sys_sched_yield();
    println!();
    println!("[hyb-image] Analise de imagem concluida (VFS do kernel).");
    sys_exit(0);
}
