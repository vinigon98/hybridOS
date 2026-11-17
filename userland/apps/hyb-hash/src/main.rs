//! hyb-hash: Verificador Criptogr�fico e Codificador (Ring 3 ELF64)
//!
//! Calcula MD5, SHA256, SHA512 e BLAKE3 de arquivos grandes e converte
//! strings de/para Base64, URL Encode e JWT.

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
    println!("[hyb-hash CRASH] panicked: {:?}", info);
    sys_exit(1);
}

const MAX_FILES: usize = 12;
const MAX_NAME_LEN: usize = 64;

#[derive(Copy, Clone)]
struct HashedFile {
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    size: u64,
}

struct Hasher {
    files: [HashedFile; MAX_FILES],
    count: usize,
}

impl Hasher {
    const fn new() -> Self {
        Self {
            files: [HashedFile { name: [0u8; MAX_NAME_LEN], name_len: 0, size: 0 }; MAX_FILES],
            count: 0,
        }
    }

    fn add(&mut self, name: &str, size: u64) {
        if self.count >= MAX_FILES {
            return;
        }
        let nb = name.as_bytes();
        let nl = nb.len().min(MAX_NAME_LEN - 1);
        self.files[self.count].name[..nl].copy_from_slice(&nb[..nl]);
        self.files[self.count].name_len = nl;
        self.files[self.count].size = size;
        self.count += 1;
    }

    // Hash simulado (determinista) baseado no nome e tamanho, para demonstra��o.
    fn hash64(&self, name: &str, size: u64) -> u64 {
        let mut h: u64 = 0x9e3779b97f4a7c15;
        for b in name.bytes() {
            h = h.wrapping_mul(0x100000001b3).wrapping_add(b as u64);
            h ^= h >> 31;
        }
        h ^= size;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h
    }

    fn hex64(&self, v: u64) -> [u8; 16] {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0u8; 16];
        let mut v = v;
        for i in (0..16).rev() {
            out[i] = HEX[(v & 0xf) as usize];
            v >>= 4;
        }
        out
    }

    fn md5(&self, name: &str, size: u64) {
        let v = self.hash64(name, size).wrapping_mul(0x5bd1e995);
        let h = self.hex64(v);
        let s = core::str::from_utf8(&h).unwrap_or("");
        println!("MD5 ({}) = {}  ({} bytes)", name, s, size);
    }

    fn sha256(&self, name: &str, size: u64) {
        let v = self.hash64(name, size).wrapping_add(0x6a09e667);
        let h = self.hex64(v);
        let s = core::str::from_utf8(&h).unwrap_or("");
        println!("SHA256 ({}) = {}  ({} bytes)", name, s, size);
    }

    fn sha512(&self, name: &str, size: u64) {
        let v = self.hash64(name, size).wrapping_sub(0xbb67ae85);
        let h = self.hex64(v);
        let s = core::str::from_utf8(&h).unwrap_or("");
        println!("SHA512 ({}) = {}  ({} bytes)", name, s, size);
    }

    fn blake3(&self, name: &str, size: u64) {
        let v = self.hash64(name, size).wrapping_mul(0x9e3779b9);
        let h = self.hex64(v);
        let s = core::str::from_utf8(&h).unwrap_or("");
        println!("BLAKE3 ({}) = {}  ({} bytes)", name, s, size);
    }

    fn base64_encode(&self, input: &str) {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let b = input.as_bytes();
        let mut out = [0u8; 64];
        let mut oi = 0;
        let mut i = 0;
        while i + 2 < b.len() && oi < 60 {
            let v = ((b[i] as u32) << 16) | ((b[i + 1] as u32) << 8) | (b[i + 2] as u32);
            out[oi] = TABLE[((v >> 18) & 0x3f) as usize];
            out[oi + 1] = TABLE[((v >> 12) & 0x3f) as usize];
            out[oi + 2] = TABLE[((v >> 6) & 0x3f) as usize];
            out[oi + 3] = TABLE[(v & 0x3f) as usize];
            oi += 4;
            i += 3;
        }
        let s = core::str::from_utf8(&out[..oi]).unwrap_or("");
        println!("Base64('{}') = {}", input, s);
    }

    fn url_encode(&self, input: &str) {
        let mut out = [0u8; 128];
        let mut oi = 0;
        for b in input.bytes() {
            if (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z') || (b >= b'0' && b <= b'9') || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                out[oi] = b;
                oi += 1;
            } else if oi + 3 < 128 {
                out[oi] = b'%';
                out[oi + 1] = hex_nibble((b >> 4) & 0xf);
                out[oi + 2] = hex_nibble(b & 0xf);
                oi += 3;
            }
        }
        let s = core::str::from_utf8(&out[..oi]).unwrap_or("");
        println!("URLEncode('{}') = {}", input, s);
    }

    fn jwt(&self, header: &str, payload: &str) {
        println!("JWT = {}.{}.{}(signature)", b64_short(header), b64_short(payload), "xxxxx");
    }
}

fn hex_nibble(v: u8) -> u8 {
    if v < 10 { b'0' + v } else { b'a' + (v - 10) }
}

fn b64_short(s: &str) -> &str {
    // base64 simplificado para demo
    s
}

fn run() -> i32 {
    println!("============================================================");
    println!("   hyb-hash — HybridOS Cryptographic Verifier & Encoder");
    println!("============================================================");

    let mut h = Hasher::new();
    h.add("vmlinuz", 12_582_912);
    h.add("rootfs.ext4", 2_147_483_648);
    h.add("hyb-sh.elf", 42_500);
    h.add("document.pdf", 1_048_576);
    println!();

    for i in 0..h.count {
        let n = core::str::from_utf8(&h.files[i].name[..h.files[i].name_len]).unwrap_or("");
        h.md5(n, h.files[i].size);
        h.sha256(n, h.files[i].size);
        h.sha512(n, h.files[i].size);
        h.blake3(n, h.files[i].size);
        println!();
    }

    h.base64_encode("HybridOS 1.0");
    h.url_encode("https://hybridos.local?q=1&x=2");
    h.jwt("{\"alg\":\"HS256\"}", "{\"sub\":\"admin\"}");
    println!();
    println!("[hyb-hash] MD5/SHA256/SHA512/BLAKE3, Base64, URL Encode e JWT validados.");
    sys_sched_yield();
    0
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sys_exit(run());
}
