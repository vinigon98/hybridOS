//! hyb-disks: Gerenciamento de Discos (ELF64)
//! Integracao com rust-kernel/src/vfs/ e rust-kernel/src/fs/ reais via syscalls.

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
    println!("[hyb-disks CRASH] {:?}", info);
    sys_exit(1);
}

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
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

fn show_disk(path: &str, label: &str) {
    let ok = probe_path(path);
    println!("  {:<12} {:<24} {}", label, path, if ok { "[ONLINE]" } else { "[OFFLINE/NAO ENCONTRADO]" });
    if ok {
        if let Some(content) = read_file(path) {
            println!("             tamanho lido via VFS: {} bytes", content.len());
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Gerenciamento de Discos (hyb-disks)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Inventario de discos e volumes", pid);

    // Discos e volumes visiveis pelo VFS do kernel (rust-kernel/src/vfs/ + fs/)
    println!();
    println!("--- Discos e volumes (VFS + fs do kernel) ---");
    show_disk("/dev/sda", "Disco 0");
    show_disk("/dev/sdb", "Disco 1");
    show_disk("/dev/nvme0n1", "NVMe 0");
    show_disk("/c", "Volume C:");
    show_disk("/d", "Volume D:");
    show_disk("/mnt", "Ponto de montagem");
    show_disk("/c/Windows/System32/ntoskrnl.ini", "Arquivo de sistema");

    // Leitura do FLTMGR (filtro de filesystem) via VFS
    println!();
    println!("--- Filtro de Sistema de Arquivos (FLTMGR) ---");
    if let Some(content) = read_file("/c/Windows/System32/ntoskrnl.ini") {
        if content.is_empty() {
            println!("  ntoskrnl.ini: aberto via VFS+fs, sem dados de volume");
        } else {
            let text = String::from_utf8_lossy(&content);
            println!("  Volume C: montado (fs: FAT32/NTFS-like)");
            print!("{}", text);
        }
    } else {
        println!("  Volume C: nao acessivel via VFS/fs");
    }

    sys_sched_yield();
    println!();
    println!("[hyb-disks] Inventario de discos concluido (integracao VFS + fs do kernel).");
    sys_exit(0);
}
