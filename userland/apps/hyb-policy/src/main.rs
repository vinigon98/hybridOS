//! hyb-policy: Gerenciador de Politica de Seguranca (ELF64)
//! Le e grava arquivos de politica no VFS (/etc/policy/).

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
    println!("[hyb-policy CRASH] {:?}", info);
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

fn probe(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Gerenciador de Politica de Seguranca");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Modulo de politica", pid);

    let policies = [
        "/etc/policy/password_policy.conf",
        "/etc/policy/audit_policy.conf",
        "/etc/policy/account_lockout.conf",
        "/etc/policy/kerberos_policy.conf",
        "/etc/policy/encryption_policy.conf",
    ];

    println!();
    println!("--- Politicas de seguranca registradas ---");
    for p in policies.iter() {
        let ok = probe(p);
        println!("  {:<45} {}", p, if ok { "[CARREGADA]" } else { "[NAO ENCONTRADA]" });
        if ok {
            if let Some(content) = read_file(p) {
                if !content.is_empty() {
                    let text = String::from_utf8_lossy(&content);
                    println!("      ({} bytes)", content.len());
                    let _ = text; // conteudo disponivel para inspecao
                }
            }
        }
    }

    // Grava uma politica de exemplo
    println!();
    println!("--- Gravando politica de exemplo ---");
    let example = b"# HybridOS Security Policy\nmin_password_length=8\nrequire_complexity=true\nmax_failed_attempts=5\nlockout_duration=300\n";
    if write_file("/etc/policy/example_policy.conf", example) {
        println!("[OK] Politica gravada no VFS: /etc/policy/example_policy.conf");
    } else {
        println!("[FALHA] Nao foi possivel gravar politica no VFS");
    }

    // Verificacao de integridade da gravacao
    if let Some(content) = read_file("/etc/policy/example_policy.conf") {
        if !content.is_empty() {
            println!("[VERIFICADO] Politica lida de volta do VFS: {} bytes", content.len());
        }
    }

    sys_sched_yield();
    println!();
    println!("[hyb-policy] Gerenciamento de politica concluido (VFS do kernel).");
    sys_exit(0);
}
