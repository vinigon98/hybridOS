#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, sys_getpid, sys_sysinfo, STDOUT_FILENO};
use libhybrid_sys::linux::SysInfo;

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
    while n > 0 { tmp[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    while pos > 0 { pos -= 1; put(core::str::from_utf8(&[tmp[pos]]).unwrap_or("")); }
}

fn put_hex_byte(v: u8, out: &mut [u8], pos: &mut usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out[*pos] = HEX[(v >> 4) as usize]; *pos += 1;
    out[*pos] = HEX[(v & 0x0f) as usize]; *pos += 1;
}

struct XorShift64 { state: u64 }
impl XorShift64 {
    fn seed(seed: u64) -> Self {
        let mut s = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        s = s.wrapping_add(0x9E3779B97F4A7C15);
        Self { state: s }
    }
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.state = x; x
    }
}

fn run() {
    put("HybridOS ssh-keygen v0.1 - Gerador Criptografico de Chaves\r\n");
    put("Gerando par de chaves ed25519 (simulado) ...\r\n");
    let mut info = SysInfo::default();
    let _rc = sys_sysinfo(&mut info);
    let entropy = sys_getpid() as u64
        .wrapping_add(info.uptime_ticks)
        .wrapping_add(info.total_mem_bytes)
        .wrapping_add(info.context_switches);
    let mut rng = XorShift64::seed(entropy);
    let mut privkey = [0u8; 32];
    for i in 0..4 {
        let w = rng.next();
        privkey[i * 4] = (w >> 24) as u8; privkey[i * 4 + 1] = (w >> 16) as u8;
        privkey[i * 4 + 2] = (w >> 8) as u8; privkey[i * 4 + 3] = w as u8;
    }
    let mut pubkey = [0u8; 32];
    let mut acc: u64 = 0xCBF29CE484222325;
    for i in 0..32 {
        acc = acc.wrapping_mul(0x100000001B3).wrapping_add(privkey[i] as u64);
        pubkey[i] = (acc >> 24) as u8;
    }
    put("entropia usada: "); put_u64(entropy); put("\r\n");
    let mut hex = [0u8; 64];
    let mut pos = 0;
    for i in 0..32 { put_hex_byte(pubkey[i], &mut hex, &mut pos); }
    put("chave publica: ssh-ed25519 ");
    let _ = sys_write(STDOUT_FILENO, &hex[..64]); put("\r\n");
    put("arquivos gravados (conceito): id_ed25519, id_ed25519.pub\r\n");
    putln("ssh-keygen: par de chaves gerado com sucesso.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
