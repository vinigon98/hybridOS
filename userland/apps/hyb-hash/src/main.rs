#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_read, sys_write, sys_exit, STDIN_FILENO, STDOUT_FILENO};

// Constantes SHA-256 (primeiros 32 bits das partes fracionarias das raizes cubicas dos primeiros 64 primos).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const INIT: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn rotr(x: u32, n: u32) -> u32 { x.rotate_right(n) }

fn sha256_block(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    let mut i = 0;
    while i < 16 {
        w[i] = (block[i * 4] as u32) << 24
            | (block[i * 4 + 1] as u32) << 16
            | (block[i * 4 + 2] as u32) << 8
            | block[i * 4 + 3] as u32;
        i += 1;
    }
    i = 16;
    while i < 64 {
        let s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
        let s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
        i += 1;
    }
    let mut a = state[0]; let mut b = state[1]; let mut c = state[2]; let mut d = state[3];
    let mut e = state[4]; let mut f = state[5]; let mut g = state[6]; let mut h = state[7];
    i = 0;
    while i < 64 {
        let s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        h = g; g = f; f = e; e = d.wrapping_add(temp1);
        d = c; c = b; b = a; a = temp1.wrapping_add(temp2);
        i += 1;
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = INIT;
    let mut i = 0;
    while i + 64 <= data.len() {
        let mut block = [0u8; 64];
        block.copy_from_slice(&data[i..i + 64]);
        sha256_block(&mut state, &block);
        i += 64;
    }
    let rem = data.len() - i;
    let mut buf = [0u8; 64];
    let mut j = 0;
    while j < rem {
        buf[j] = data[i + j];
        j += 1;
    }
    buf[rem] = 0x80;
    let len_bits = (data.len() as u64) * 8;
    if rem + 1 + 8 <= 64 {
        let mut k = 0;
        while k < 8 {
            buf[56 + k] = ((len_bits >> (56 - k * 8)) & 0xff) as u8;
            k += 1;
        }
        sha256_block(&mut state, &buf);
    } else {
        let mut k = rem + 1;
        while k < 64 { buf[k] = 0; k += 1; }
        sha256_block(&mut state, &buf);
        let mut block2 = [0u8; 64];
        k = 0;
        while k < 8 {
            block2[56 + k] = ((len_bits >> (56 - k * 8)) & 0xff) as u8;
            k += 1;
        }
        sha256_block(&mut state, &block2);
    }
    let mut out = [0u8; 32];
    i = 0;
    while i < 8 {
        out[i * 4] = (state[i] >> 24) as u8;
        out[i * 4 + 1] = (state[i] >> 16) as u8;
        out[i * 4 + 2] = (state[i] >> 8) as u8;
        out[i * 4 + 3] = state[i] as u8;
        i += 1;
    }
    out
}

fn put(s: &str) {
    let _ = sys_write(STDOUT_FILENO, s.as_bytes());
}
fn putln(s: &str) {
    put(s);
    put("\r\n");
}
fn put_u64(v: u64) {
    if v == 0 { put("0"); return; }
    let mut tmp = [0u8; 20];
    let mut n = v;
    let mut pos = 0;
    while n > 0 {
        tmp[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    while pos > 0 {
        pos -= 1;
        put(core::str::from_utf8(&[tmp[pos]]).unwrap_or(""));
    }
}

fn run() {
    put("HybridOS hyb-hash v0.1 - Verificador Criptografico e Codificador\r\n");
    put("algoritmo: SHA-256\r\n");
    put("lendo entrada padrao...\r\n");

    let mut data = [0u8; 4096];
    let mut total = 0usize;
    loop {
        let n = sys_read(STDIN_FILENO, &mut data[total..]);
        if n <= 0 { break; }
        total += n as usize;
        if total >= data.len() { break; }
    }

    let digest = sha256(&data[..total]);

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0u8; 64];
    let mut i = 0;
    while i < 32 {
        hex[i * 2] = HEX[(digest[i] >> 4) as usize];
        hex[i * 2 + 1] = HEX[(digest[i] & 0x0f) as usize];
        i += 1;
    }
    put("sha256(");
    put_u64(total as u64);
    put(" bytes) = ");
    let _ = sys_write(STDOUT_FILENO, &hex);
    put("\r\n");
    putln("hyb-hash: digest gerado.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
