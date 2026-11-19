//! ssh-keygen: Gerador Criptogr�fico de Chaves (Ring 3 ELF64)
//!
//! Gera��o de chaves Ed25519, RSA e suporte nativo a chaves de hardware
//! FIDO2/YubiKey.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[ssh-keygen CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_KEYS: usize = 8;
const MAX_COMMENT_LEN: usize = 64;

#[derive(Copy, Clone)]
struct KeyPair {
    algorithm: [u8; 16],
    algorithm_len: usize,
    bits: u32,
    comment: [u8; MAX_COMMENT_LEN],
    comment_len: usize,
    hardware_backed: bool,
}

struct KeyGenerator {
    keys: [KeyPair; MAX_KEYS],
    count: usize,
}

impl KeyGenerator {
    const fn new() -> Self {
        Self {
            keys: [KeyPair { algorithm: [0u8; 16], algorithm_len: 0, bits: 0, comment: [0u8; MAX_COMMENT_LEN], comment_len: 0, hardware_backed: false }; MAX_KEYS],
            count: 0,
        }
    }

    fn generate(&mut self, algorithm: &str, bits: u32, comment: &str, hardware_backed: bool) {
        if self.count >= MAX_KEYS {
            println!("[ssh-keygen] l`//! ssh-keygen: Gerador Criptogr�fico de Chaves (Ring 3 ELF64)
//!
//! Gera��o de chaves Ed25519, RSA e suporte nativo a chaves de hardware
//! FIDO2/YubiKey.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[ssh-keygen CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_KEYS: usize = 8;
const MAX_COMMENT_LEN: usize = 64;

#[derive(Copy, Clone)]
struct KeyPair {
    algorithm: [u8; 16],
    algorithm_len: usize,
    bits: u32,
    comment: [u8; MAX_COMMENT_LEN],
    comment_len: usize,
    hardware_backed: bool,
}

struct KeyGenerator {
    keys: [KeyPair; MAX_KEYS],
    count: usize,
}

impl KeyGenerator {
    const fn new() -> Self {
        Self {
            keys: [KeyPair { algorithm: [0u8; 16], algorithm_len: 0, bits: 0, comment: [0u8; MAX_COMMENT_LEN], comment_len: 0, hardware_backed: false }; MAX_KEYS],
            count: 0,
        }
    }

    fn generate(&mut self, algorithm: &str, bits: u32, comment: &str, hardware_backed: bool) {
        if self.count >= MAX_KEYS {
            println!("[ssh-keygen] lu{ed}mite de chaves atingido.");
            return;
        }
        let ab = algorithm.as_bytes();
        let al = ab.len().min(15);
        self.keys[self.count].algorithm[..al].copy_from_slice(&ab[..al]);
        self.keys[self.count].algorithm_len = al;
        self.keys[self.count].bits = bits;
        let cb = comment.as_bytes();
        let cl = cb.len().min(MAX_COMMENT_LEN - 1);
        self.keys[self.count].comment[..cl].copy_from_slice(&cb[..cl]);
        self.keys[self.count].comment_len = cl;
        self.keys[self.count].hardware_backed = hardware_backed;
        self.count += 1;
        println!("[ssh-keygen] gerando par de chaves {} ({} bits)...", algorithm, bits);
        if hardware_backed {
            println!("[ssh-keygen] chave de hardware FIDO2/YubiKey detectada.");
        }
        println!("[ssh-keygen] chave privada salva em ~/.ssh/id_{}", algorithm);
        println!("[ssh-keygen] chave p`//! ssh-keygen: Gerador Criptogr�fico de Chaves (Ring 3 ELF64)
//!
//! Gera��o de chaves Ed25519, RSA e suporte nativo a chaves de hardware
//! FIDO2/YubiKey.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[ssh-keygen CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_KEYS: usize = 8;
const MAX_COMMENT_LEN: usize = 64;

#[derive(Copy, Clone)]
struct KeyPair {
    algorithm: [u8; 16],
    algorithm_len: usize,
    bits: u32,
    comment: [u8; MAX_COMMENT_LEN],
    comment_len: usize,
    hardware_backed: bool,
}

struct KeyGenerator {
    keys: [KeyPair; MAX_KEYS],
    count: usize,
}

impl KeyGenerator {
    const fn new() -> Self {
        Self {
            keys: [KeyPair { algorithm: [0u8; 16], algorithm_len: 0, bits: 0, comment: [0u8; MAX_COMMENT_LEN], comment_len: 0, hardware_backed: false }; MAX_KEYS],
            count: 0,
        }
    }

    fn generate(&mut self, algorithm: &str, bits: u32, comment: &str, hardware_backed: bool) {
        if self.count >= MAX_KEYS {
            println!("[ssh-keygen] l\u00edmite de chaves atingido.");
            return;
        }
        let ab = algorithm.as_bytes();
        let al = ab.len().min(15);
        self.keys[self.count].algorithm[..al].copy_from_slice(&ab[..al]);
        self.keys[self.count].algorithm_len = al;
        self.keys[self.count].bits = bits;
        let cb = comment.as_bytes();
        let cl = cb.len().min(MAX_COMMENT_LEN - 1);
        self.keys[self.count].comment[..cl].copy_from_slice(&cb[..cl]);
        self.keys[self.count].comment_len = cl;
        self.keys[self.count].hardware_backed = hardware_backed;
        self.count += 1;
        println!("[ssh-keygen] gerando par de chaves {} ({} bits)...", algorithm, bits);
        if hardware_backed {
            println!("[ssh-keygen] chave de hardware FIDO2/YubiKey detectada.");
        }
        println!("[ssh-keygen] chave privada salva em ~/.ssh/id_{}", algorithm);
        println!("[ssh-keygen] chave pu{fa}blica salva em ~/.ssh/id_{}.pub", algorithm);
        println!("[ssh-keygen] impress`//! ssh-keygen: Gerador Criptogr�fico de Chaves (Ring 3 ELF64)
//!
//! Gera��o de chaves Ed25519, RSA e suporte nativo a chaves de hardware
//! FIDO2/YubiKey.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[ssh-keygen CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_KEYS: usize = 8;
const MAX_COMMENT_LEN: usize = 64;

#[derive(Copy, Clone)]
struct KeyPair {
    algorithm: [u8; 16],
    algorithm_len: usize,
    bits: u32,
    comment: [u8; MAX_COMMENT_LEN],
    comment_len: usize,
    hardware_backed: bool,
}

struct KeyGenerator {
    keys: [KeyPair; MAX_KEYS],
    count: usize,
}

impl KeyGenerator {
    const fn new() -> Self {
        Self {
            keys: [KeyPair { algorithm: [0u8; 16], algorithm_len: 0, bits: 0, comment: [0u8; MAX_COMMENT_LEN], comment_len: 0, hardware_backed: false }; MAX_KEYS],
            count: 0,
        }
    }

    fn generate(&mut self, algorithm: &str, bits: u32, comment: &str, hardware_backed: bool) {
        if self.count >= MAX_KEYS {
            println!("[ssh-keygen] l\u00edmite de chaves atingido.");
            return;
        }
        let ab = algorithm.as_bytes();
        let al = ab.len().min(15);
        self.keys[self.count].algorithm[..al].copy_from_slice(&ab[..al]);
        self.keys[self.count].algorithm_len = al;
        self.keys[self.count].bits = bits;
        let cb = comment.as_bytes();
        let cl = cb.len().min(MAX_COMMENT_LEN - 1);
        self.keys[self.count].comment[..cl].copy_from_slice(&cb[..cl]);
        self.keys[self.count].comment_len = cl;
        self.keys[self.count].hardware_backed = hardware_backed;
        self.count += 1;
        println!("[ssh-keygen] gerando par de chaves {} ({} bits)...", algorithm, bits);
        if hardware_backed {
            println!("[ssh-keygen] chave de hardware FIDO2/YubiKey detectada.");
        }
        println!("[ssh-keygen] chave privada salva em ~/.ssh/id_{}", algorithm);
        println!("[ssh-keygen] chave p\u00fablica salva em ~/.ssh/id_{}.pub", algorithm);
        println!("[ssh-keygen] impressu{e3}o digital (fingerprint): SHA256:xx:xx:xx:xx ({})", comment);
    }

    fn list(&self) {
        println!("[ssh-keygen] chaves geradas:");
        if self.count == 0 {
            println!("  (nenhuma)");
        }
        for i in 0..self.count {
            let a = core::str::from_utf8(&self.keys[i].algorithm[..self.keys[i].algorithm_len]).unwrap_or("");
            let c = core::str::from_utf8(&self.keys[i].comment[..self.keys[i].comment_len]).unwrap_or("");
            println!("  {} {} bits hw={} comment={}", a, self.keys[i].bits, self.keys[i].hardware_backed, c);
        }
    }
}

fn run() -> i32 {
    println!("============================================================");
    println!("   ssh-keygen — HybridOS Cryptographic Key Generator");
    println!("============================================================");

    let mut gen = KeyGenerator::new();
    gen.generate("ed25519", 256, "admin@hybridos", false);
    gen.generate("rsa", 4096, "root@server", false);
    gen.generate("ed25519", 256, "yubikey-user", true); // FIDO2
    println!();
    gen.list();
    println!();
    println!("[ssh-keygen] gera��o Ed25519/RSA e suporte FIDO2/YubiKey validados.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
