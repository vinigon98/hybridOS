//! hyb-clean: Limpeza de Disco (ELF64)
//! Limpa arquivos temporarios no /tmp sobrescrevendo via VFS (sys_open/sys_write/sys_close).

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
    println!("[hyb-clean CRASH] {:?}", info);
    sys_exit(1);
}

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

fn overwrite_file(path: &str) -> bool {
    // Sobrescreve o arquivo com conteudo vazio (limpando-o) via VFS
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return false,
    };
    let empty: [u8; 0] = [];
    let _ = sys_write(fd, &empty);
    let _ = sys_close(fd);
    true
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Limpeza de Disco (hyb-clean)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Limpador de arquivos temporarios", pid);

    // Arquivos temporarios candidatos a limpeza no VFS do kernel
    let temp_files = [
        "/tmp/installer.log",
        "/tmp/update_cache.bin",
        "/tmp/crash_dump.dmp",
        "/tmp/installer_temp.dat",
        "/tmp/setup.log",
    ];

    println!();
    println!("--- Varredura de arquivos temporarios (/tmp) ---");
    let mut cleaned: u32 = 0;
    let mut skipped: u32 = 0;
    for f in temp_files.iter() {
        if probe_path(f) {
            if overwrite_file(f) {
                println!("  [LIMPO]  {}", f);
                cleaned += 1;
            } else {
                println!("  [FALHA]  {}", f);
                skipped += 1;
            }
        } else {
            println!("  [--]     {} (nao existe no VFS)", f);
            skipped += 1;
        }
    }

    // Criar marcador de limpeza no VFS
    let marker = b"hyb-clean executado em /tmp\n";
    let fd = match sys_open("/tmp/.clean_marker", 0, 0) {
        r if r >= 0 => r as usize,
        _ => 0,
    };
    if fd > 0 {
        let _ = sys_write(fd, marker);
        let _ = sys_close(fd);
        println!("  [MARCADOR] /tmp/.clean_marker gravado no VFS");
    }

    println!();
    println!("  Total limpo: {} | Total pulado: {}", cleaned, skipped);

    sys_sched_yield();
    println!();
    println!("[hyb-clean] Limpeza de disco concluida (VFS do kernel).");
    sys_exit(0);
}
