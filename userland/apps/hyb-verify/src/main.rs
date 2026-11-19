//! hyb-verify: Verificacao de Integridade (ELF64)
//! Calcula hash (FNV-1a 64-bit) de arquivos via VFS e compara com valor esperado.

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
    println!("[hyb-verify CRASH] {:?}", info);
    sys_exit(1);
}

/// FNV-1a 64-bit hash (algoritmo de hash real, RFC-style).
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data.iter() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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

fn u64_to_hex(mut val: u64) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::new();
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        s.push(DIGITS[nibble] as char);
    }
    s
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Verificacao de Integridade (hyb-verify)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Motor de verificacao de integridade", pid);

    // Arquivos do sistema para verificar integridade via VFS
    let targets = [
        "/etc/os-release",
        "/c/Windows/System32/ntoskrnl.ini",
        "/dev/null",
        "/dev/zero",
    ];

    println!();
    println!("--- Hashes de integridade (FNV-1a 64-bit) via VFS ---");
    for t in targets.iter() {
        if let Some(content) = read_file(t) {
            let hash = fnv1a_64(&content);
            println!("  {:<38} hash=0x{} tamanho={}", t, u64_to_hex(hash), content.len());
        } else {
            println!("  {:<38} NAO ACESSIVEL via VFS", t);
        }
    }

    // Comparacao com hash esperado (modo verificacao)
    println!();
    println!("--- Modo de verificacao (comparacao com hash esperado) ---");
    let expected_path = "/etc/os-release";
    let expected_hash_hex = "0000000000000000"; // placeholder; em produção viria de um manifesto assinado
    if let Some(content) = read_file(expected_path) {
        let actual = u64_to_hex(fnv1a_64(&content));
        let match_ok = actual == expected_hash_hex;
        println!("  Arquivo: {}", expected_path);
        println!("  Hash calculado: {}", actual);
        println!("  Hash esperado:  {}", expected_hash_hex);
        println!("  Resultado: {}", if match_ok { "CORRESPONDE" } else { "NAO CORRESPONDE (hash de referencia nao disponível)" });
    } else {
        println!("  Nao foi possivel ler {} para verificacao", expected_path);
    }

    sys_sched_yield();
    println!();
    println!("[hyb-verify] Verificacao de integridade concluida (hashes via VFS do kernel).");
    sys_exit(0);
}
