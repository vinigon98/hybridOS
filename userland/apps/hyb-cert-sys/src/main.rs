//! hyb-cert-sys: Gestor de Autoridades Raiz do Sistema (Ring 3 ELF64)
//! Repositorio central de CAs do SO, adiciona CA localhost auto-assinado para HTTPS dev.

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
    println!("[hyb-cert-sys CRASH] {:?}", info);
    sys_exit(1);
}

const MAX_CA: usize = 8;
const MAX_SUBJ: usize = 48;

#[derive(Copy, Clone)]
struct RaizCa {
    assunto: [u8; MAX_SUBJ],
    assunto_len: usize,
    confiavel: bool,
    impressao: u64,
}

struct TrustStore {
    raizes: [RaizCa; MAX_CA],
    count: usize,
}

impl TrustStore {
    fn novo() -> Self {
        let mut raizes = [RaizCa {
            assunto: [0; MAX_SUBJ],
            assunto_len: 0,
            confiavel: false,
            impressao: 0,
        }; MAX_CA];
        let mut i = 0;
        while i < MAX_CA {
            raizes[i] = RaizCa { assunto: [0; MAX_SUBJ], assunto_len: 0, confiavel: false, impressao: 0 };
            i += 1;
        }
        Self { raizes, count: 0 }
    }

    fn adicionar(&mut self, assunto: &str, confiavel: bool) {
        if self.count >= MAX_CA {
            return;
        }
        let b = assunto.as_bytes();
        let len = b.len().min(MAX_SUBJ - 1);
        let mut h: u64 = 0xcbf29ce484222325;
        let mut i = 0;
        while i < len {
            h ^= b[i] as u64;
            h = h.wrapping_mul(0x100000001b3);
            i += 1;
        }
        self.raizes[self.count] = RaizCa { assunto: [0; MAX_SUBJ], assunto_len: 0, confiavel, impressao: h };
        self.raizes[self.count].assunto[..len].copy_from_slice(&b[..len]);
        self.raizes[self.count].assunto_len = len;
        self.count += 1;
    }

    fn contem(&self, assunto: &str) -> bool {
        let b = assunto.as_bytes();
        let mut i = 0;
        while i < self.count {
            let r = &self.raizes[i];
            if r.assunto_len == b.len() && r.assunto[..b.len()] == b[..] {
                return true;
            }
            i += 1;
        }
        false
    }

    fn listar(&self) {
        let mut i = 0;
        while i < self.count {
            let r = &self.raizes[i];
            let s = core::str::from_utf8(&r.assunto[..r.assunto_len]).unwrap_or("");
            println!("  - {}  confiavel={}  impressao=0x{:016X}", s, r.confiavel, r.impressao);
            i += 1;
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-cert-sys - Gestor de Autoridades Raiz do Sistema   ");
    println!("========================================================");
    let mut store = TrustStore::novo();
    store.adicionar("CN=HybridOS Root CA, O=Hybridos, C=BR", true);
    store.adicionar("CN=localhost Dev CA, O=Dev, C=BR", true);
    store.adicionar("CN=CA Externa Exemplo, O=Exemplo, C=US", false);
    println!("[hyb-cert-sys] Autoridades raiz no repositorio do sistema:");
    store.listar();
    println!("[hyb-cert-sys] Contem 'localhost Dev CA'? {}", store.contem("CN=localhost Dev CA, O=Dev, C=BR"));
    println!("[hyb-cert-sys] Contem 'ca.falsa'? {}", store.contem("CN=ca.falsa, O=X, C=BR"));
    println!("[hyb-cert-sys] Repositorio de raizes finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
