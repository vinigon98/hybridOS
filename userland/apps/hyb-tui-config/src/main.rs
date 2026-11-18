//! hyb-tui-config: Editor TUI de Configuracao (ELF64)
//! Lista, edita e grava arquivos de configuracao em /etc/ via VFS do kernel.

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
    println!("[hyb-tui-config CRASH] {:?}", info);
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

fn write_file(path: &str, data: &[u8]) -> bool {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return false,
    };
    let mut written = 0usize;
    while written < data.len() {
        let n = sys_write(fd, &data[written..]);
        if n <= 0 {
            break;
        }
        written += n as usize;
    }
    let _ = sys_close(fd);
    written == data.len()
}

fn list_etc() {
    println!("Arquivos de configuracao em /etc/:");
    let known = [
        "/etc/os-release",
        "/etc/hostname",
        "/etc/network.conf",
        "/etc/firewall.conf",
        "/etc/passwd",
        "/etc/services",
    ];
    for p in known.iter() {
        if read_file(p).is_some() {
            println!("  [OK]  {}", p);
        } else {
            println!("  [--]  {} (nao encontrado)", p);
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS TUI Config Editor");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Editor de configuracao TUI", pid);

    // Lista configuracoes disponiveis no VFS
    list_etc();

    // Le /etc/os-release se existir
    println!();
    println!("--- Conteudo de /etc/os-release ---");
    if let Some(content) = read_file("/etc/os-release") {
        if content.is_empty() {
            println!("  (arquivo aberto via VFS, sem conteudo lido)");
        } else {
            let text = String::from_utf8_lossy(&content);
            print!("{}", text);
        }
    } else {
        println!("  Nao foi possivel abrir /etc/os-release via VFS");
    }

    // Grava um arquivo de configuracao de exemplo via VFS
    println!("--- Gravando configuracao de exemplo ---");
    let cfg = b"# HybridOS TUI Config\nlocale=pt_BR\nkeyboard=abnt2\ntheme=crystal_glass\n";
    if write_file("/etc/hyb-tui-config.conf", cfg) {
        println!("[OK] /etc/hyb-tui-config.conf gravado via VFS (sys_open/sys_write/sys_close)");
    } else {
        println!("[FALHA] Nao foi possivel gravar /etc/hyb-tui-config.conf");
    }

    // Relendo para verificar gravacao
    if let Some(content) = read_file("/etc/hyb-tui-config.conf") {
        if !content.is_empty() {
            let text = String::from_utf8_lossy(&content);
            println!("--- Verificacao (relendo via VFS) ---");
            print!("{}", text);
        }
    }

    sys_sched_yield();
    println!("[hyb-tui-config] Operacoes de configuracao via VFS concluídas.");
    sys_exit(0);
}
