//! hyb-crypt: Ferramenta de Criptografia (ELF64)
//! Cifra XOR e hash (FNV-1a) de arquivos via VFS do kernel.

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
    println!("[hyb-crypt CRASH] {:?}", info);
    sys_exit(1);
}

/// FNV-1a 64-bit hash.
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data.iter() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Cifra XOR de fluxo com chave derivada de senha.
fn xor_crypt(data: &mut [u8], key: &[u8]) {
    if key.is_empty() {
        return;
    }
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= key[i % key.len()];
    }
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
    println!("   HybridOS Ferramenta de Criptografia (hyb-crypt)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Motor de criptografia do sistema", pid);

    let source = "/etc/os-release";
    let encrypted = "/tmp/os-release.enc";
    let decrypted = "/tmp/os-release.dec";
    let password = b"hybridos_secret_key";

    println!();
    println!("--- Operacao de criptografia (cifra XOR de fluxo) ---");

    // Ler arquivo de origem via VFS
    if let Some(content) = read_file(source) {
        if content.is_empty() {
            println!("  {}: aberto via VFS, conteudo vazio", source);
        } else {
            println!("  Lido {}: {} bytes via VFS", source, content.len());

            // Criptografar
            let mut ciphertext = content.clone();
            xor_crypt(&mut ciphertext, password);
            let hash_ct = u64_to_hex(fnv1a_64(&ciphertext));
            println!("  Hash do ciphertext (FNV-1a): {}", hash_ct);

            // Gravar cifrado no VFS
            if write_file(encrypted, &ciphertext) {
                println!("  [OK] Cifrado gravado em {} via VFS", encrypted);
            } else {
                println!("  [FALHA] Nao foi possivel gravar arquivo cifrado");
            }

            // Decriptografar (XOR e simetrica)
            let mut plaintext = ciphertext.clone();
            xor_crypt(&mut plaintext, password);
            let hash_pt = u64_to_hex(fnv1a_64(&plaintext));
            println!("  Hash do texto claro (FNV-1a): {}", hash_pt);

            // Gravar decriptografado
            if write_file(decrypted, &plaintext) {
                println!("  [OK] Decriptografado gravado em {} via VFS", decrypted);
            }

            // Verificacao de round-trip
            let roundtrip_ok = plaintext == content;
            println!("  Round-trip decriptografia: {}", if roundtrip_ok { "SUCESSO" } else { "FALHA" });
        }
    } else {
        println!("  Nao foi possivel ler {} via VFS", source);
    }

    sys_sched_yield();
    println!();
    println!("[hyb-crypt] Operacoes de criptografia concluídas (VFS do kernel).");
    sys_exit(0);
}
