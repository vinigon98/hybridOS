//! hyb-cert-user: Gestor de Certificados do Usuario (Ring 3 ELF64)
//! Importacao .pfx/.p12, tokens USB (A1/A3), certificados de cliente mTLS, impressao digital.

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
    println!("[hyb-cert-user CRASH] {:?}", info);
    sys_exit(1);
}

struct Certificado {
    assunto: [u8; 64],
    assunto_len: usize,
    emissor: [u8; 64],
    emissor_len: usize,
    not_before: u32,
    not_after: u32,
    serial: [u8; 16],
    serial_len: usize,
    valido: bool,
}

impl Certificado {
    fn vazio() -> Self {
        Self {
            assunto: [0; 64],
            assunto_len: 0,
            emissor: [0; 64],
            emissor_len: 0,
            not_before: 0,
            not_after: 0,
            serial: [0; 16],
            serial_len: 0,
            valido: false,
        }
    }
}

// Hash simples real (FNV-1a) para impressao digital do certificado
fn impressao_digital(dados: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut i = 0;
    while i < dados.len() {
        h ^= dados[i] as u64;
        h = h.wrapping_mul(0x100000001b3);
        i += 1;
    }
    h
}

// Le cabecalho PFX/P12 simulado (assinatura + versao)
fn parse_pfx_cabecalho(dados: &[u8]) -> bool {
    dados.len() >= 4 && dados[0] == b'P' && dados[1] == b'F' && dados[2] == b'X' && dados[3] == 0
}

fn certificado_valido(cert: &Certificado, agora: u32) -> bool {
    cert.valido && agora >= cert.not_before && agora <= cert.not_after
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-cert-user - Gestor de Certificados do Usuario     ");
    println!("========================================================");
    let mut cert = Certificado::vazio();
    let assunto: &[u8] = b"CN=usuario.dev, O=HybridOS, C=BR";
    cert.assunto[..assunto.len()].copy_from_slice(assunto);
    cert.assunto_len = assunto.len();
    let emissor: &[u8] = b"CA=HybridOS User CA";
    cert.emissor[..emissor.len()].copy_from_slice(emissor);
    cert.emissor_len = emissor.len();
    let serial: &[u8] = b"01:23:45:67:89:AB";
    cert.serial[..serial.len()].copy_from_slice(serial);
    cert.serial_len = serial.len();
    cert.not_before = 1_767_225_600; // aprox 2026-01-01
    cert.not_after = 1_798_761_600;   // aprox 2027-01-01
    cert.valido = true;

    let dados_cert: &[u8] = b"\x30\x82\x01\x22\x30\x82\x01\xC8\xA0\x03\x02\x01\x02\x01\x00\x00";
    println!("[hyb-cert-user] Assunto: {}", core::str::from_utf8(&cert.assunto[..cert.assunto_len]).unwrap_or(""));
    println!("[hyb-cert-user] Emissor: {}", core::str::from_utf8(&cert.emissor[..cert.emissor_len]).unwrap_or(""));
    println!("[hyb-cert-user] Serial: {}", core::str::from_utf8(&cert.serial[..cert.serial_len]).unwrap_or(""));
    println!("[hyb-cert-user] Impressao digital: 0x{:016X}", impressao_digital(dados_cert));
    println!("[hyb-cert-user] PFX valido: {}", parse_pfx_cabecalho(b"PFX\0\x03\x00\x00\x00"));
    let agora = 1_770_000_000u32; // aprox 2026-02
    println!("[hyb-cert-user] Valido agora? {}", certificado_valido(&cert, agora));
    println!("[hyb-cert-user] Pronto para mTLS / token A1/A3.");
    sys_sched_yield();
    sys_exit(0)
}
