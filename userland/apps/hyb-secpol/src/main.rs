//! hyb-secpol: Politica de Securanca (SecPol) (ELF64)
//! Gerencia políticas de segurança do sistema com persistência em VFS.

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
    println!("[hyb-secpol CRASH] {:?}", info);
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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS SecPol - Politica de Securanca");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | SecPol engine", pid);

    let secpol_paths = [
        "/etc/secpol/system_secpol.conf",
        "/etc/secpol/network_secpol.conf",
        "/etc/secpol/file_system_secpol.conf",
        "/etc/secpol/user_rights_secpol.conf",
    ];

    println!();
    println!("--- Politicas SecPol atuais ---");
    for p in secpol_paths.iter() {
        if let Some(content) = read_file(p) {
            if content.is_empty() {
                println!("  {} (aberto via VFS, vazio)", p);
            } else {
                let text = String::from_utf8_lossy(&content);
                println!("  {}: {} bytes", p, content.len());
                print!("{}", text);
            }
        } else {
            println!("  {} (nao encontrado, criando padrao...)", p);
            let def = b"# SecPol padrao HybridOS\nenabled=true\naudit_level=info\n";
            if write_file(p, def) {
                println!("    [OK] Politica padrao gravada via VFS");
            }
        }
    }

    // Politica mestre do sistema
    println!();
    println!("--- Política Mestre (System SecPol) ---");
    let master = b"# HybridOS System Security Policy\nmode=enforcing\naudit=on\ntpm=on\nsecure_boot=on\n";
    if write_file("/etc/secpol/system_secpol.conf", master) {
        println!("[OK] SecPol mestre gravado no VFS do kernel.");
    }

    sys_sched_yield();
    println!();
    println!("[hyb-secpol] Politicas de seguranca gerenciadas (VFS do kernel).");
    sys_exit(0);
}
