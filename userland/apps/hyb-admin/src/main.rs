//! hyb-admin: Ferramenta de Administracao do Sistema (ELF64)
//! Painel administrativo: VFS + escalonador do kernel (sched).

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
    println!("[hyb-admin CRASH] {:?}", info);
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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Administracao do Sistema (hyb-admin)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID do administrador: {}", pid);

    // Inventario de caminhos do VFS (arquivos reais do kernel)
    println!();
    println!("--- Inventario do VFS do kernel ---");
    let vfs_paths = [
        "/etc/os-release",
        "/etc/hostname",
        "/bin",
        "/sbin",
        "/dev",
        "/dev/null",
        "/dev/zero",
        "/dev/console",
        "/c/Windows/System32/ntoskrnl.ini",
        "/tmp",
        "/mnt",
    ];
    for p in vfs_paths.iter() {
        let ok = probe_path(p);
        println!("  {:<40} {}", p, if ok { "[OK]" } else { "[--]" });
    }

    // Estatisticas de leitura de arquivos chave
    println!();
    println!("--- Leitura de arquivos de sistema ---");
    if let Some(content) = read_file("/etc/os-release") {
        println!("/etc/os-release: {} bytes lidos via VFS", content.len());
    } else {
        println!("/etc/os-release: nao acessivel via VFS");
    }

    // Interacao com escalonador: cede CPU e re-cede (sched real)
    println!();
    println!("--- Escalonador (sched) ---");
    println!("Executando sys_sched_yield() x3...");
    sys_sched_yield();
    sys_sched_yield();
    sys_sched_yield();
    println!("3 sys_sched_yield() enviados ao escalonador do kernel.");

    println!();
    println!("[hyb-admin] Painel administrativo concluido (VFS + sched do kernel).");
    sys_exit(0);
}
