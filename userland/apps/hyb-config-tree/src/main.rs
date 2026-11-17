//! hyb-config-tree: Visualizador de Arvore de Configuracao (ELF64)
//! Percorre a arvore de diretorios do VFS (/etc, /bin, /sbin, /dev).

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
    println!("[hyb-config-tree CRASH] {:?}", info);
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

fn show_node(path: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let ok = probe_path(path);
    println!("{}-> {} [{}]", indent, path, if ok { "OK" } else { "--" });
    if ok {
        if let Some(content) = read_file(path) {
            if !content.is_empty() && content.len() < 200 {
                let text = String::from_utf8_lossy(&content);
                println!("{}   | conteudo: {}", indent, text.trim().chars().take(80).collect::<String>());
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Arvore de Configuracao (hyb-config-tree)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {}", pid);
    println!("Percorrendo arvore do VFS do kernel...\n");

    // Raiz e diretorios de sistema
    println!("  /");
    for dir in ["/etc", "/bin", "/sbin", "/dev", "/tmp", "/mnt", "/c", "/c/Windows", "/c/Windows/System32"] {
        show_node(dir, 1);
    }

    // Arquivos de configuracao dentro de /etc
    println!();
    println!("  /etc (arquivos):");
    for f in ["/etc/os-release", "/etc/hostname", "/etc/network.conf", "/etc/firewall.conf", "/etc/passwd", "/etc/services", "/etc/hosts"] {
        show_node(f, 2);
    }

    // Dispositivos em /dev
    println!();
    println!("  /dev (dispositivos):");
    for d in ["/dev/null", "/dev/zero", "/dev/console", "/dev/tty0", "/dev/sda", "/dev/sdb"] {
        show_node(d, 2);
    }

    // Hierarquia Windows NT
    println!();
    println!("  /c/Windows/System32 (binarios NT):");
    for f in ["/c/Windows/System32/ntoskrnl.ini", "/c/Windows/System32/cmd.exe", "/c/Windows/System32/ntdll.dll"] {
        show_node(f, 2);
    }

    sys_sched_yield();
    println!();
    println!("[hyb-config-tree] Arvore do VFS percorrida com sucesso.");
    sys_exit(0);
}
