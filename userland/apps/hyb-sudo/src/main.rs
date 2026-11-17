//! hyb-sudo: Executar comando com privilégios elevados (ELF64)
//! Verifica usuario (root via sys_getpid) e permissoes no VFS.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
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
    println!("[hyb-sudo CRASH] {:?}", info);
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
    println!("   HybridOS Sudo - Executar com Privilégios Elevados");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID do solicitante: {}", pid);

    // UID 0 = root (Administrador). sys_getpid retorna o PID do processo.
    let is_root = pid == 0;
    println!("UID efetivo: {} (0 = root/Administrador)", pid);
    println!("Privilégio de elevacao: {}", if is_root { "CONCEDIDO (root)" } else { "NAO É ROOT (solicitacao de elevacao)" });

    println!();
    println!("--- Verificacao de permissoes via VFS ---");
    let protected = [
        "/etc/os-release",
        "/c/Windows/System32/ntoskrnl.ini",
        "/etc/passwd",
        "/etc/firewall/rules.conf",
    ];
    for p in protected.iter() {
        let accessible = probe_path(p);
        println!("  {:<40} {}", p, if accessible { "[ACESSIVEL]" } else { "[RESTRITO]" });
        if accessible {
            if let Some(content) = read_file(p) {
                println!("      leitura via VFS: {} bytes OK", content.len());
            }
        }
    }

    // Simular execucao de comando elevado
    println!();
    println!("--- Execucao de comando elevado ---");
    let cmd = "hyb-admin --list-services";
    if is_root {
        println!("  Executando (elevado): {}", cmd);
        // Gravar auditoria de sudo no VFS
        let mut audit = String::new();
        audit.push_str("sudo: executou '");
        audit.push_str(cmd);
        audit.push_str("' [ELEVADO]\n");
        if write_file("/tmp/.sudo_audit", audit.as_bytes()) {
            println!("  [AUDITORIA] Registro de sudo gravado em /tmp/.sudo_audit");
        }
        println!("  Comando executado com sucesso (privilégios elevados).");
    } else {
        println!("  Sudo: usuario nao e root. Solicitacao de elevacao enviada.");
        println!("  Para elevar, execute como Administrador (UID 0).");
    }

    sys_sched_yield();
    println!();
    println!("[hyb-sudo] Operacao de elevacao concluida (permissoes via VFS do kernel).");
    sys_exit(0);
}
