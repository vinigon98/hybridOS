//! hyb-clip: Daemon de Area de Transferencia (Ring 3 ELF64)
//! Historico de clipboard em RAM, texto/imagem/caminhos, itens fixados e limpeza seletiva.

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
    println!("[hyb-clip CRASH] {:?}", info);
    sys_exit(1);
}

const MAX_ITEMS: usize = 16;
const MAX_LEN: usize = 64;

#[derive(Copy, Clone)]
enum Kind {
    Texto,
    Imagem,
    Caminho,
}

#[derive(Copy, Clone)]
struct Item {
    kind: Kind,
    dados: [u8; MAX_LEN],
    len: usize,
    fixado: bool,
    timestamp: u64,
}

impl Item {
    fn vazio() -> Self {
        Self { kind: Kind::Texto, dados: [0; MAX_LEN], len: 0, fixado: false, timestamp: 0 }
    }

    fn mostrar(&self, idx: usize) {
        let s = core::str::from_utf8(&self.dados[..self.len]).unwrap_or("");
        let k = match self.kind {
            Kind::Texto => "TXT",
            Kind::Imagem => "IMG",
            Kind::Caminho => "PTH",
        };
        let fix = if self.fixado { " [FIXO]" } else { "" };
        println!("  [{}] {}: {} (t={}{})", idx, k, s, self.timestamp, fix);
    }
}

struct Clipboard {
    itens: [Item; MAX_ITEMS],
    count: usize,
    relogio: u64,
}

impl Clipboard {
    fn novo() -> Self {
        let mut itens = [Item::vazio(); MAX_ITEMS];
        let mut i = 0;
        while i < MAX_ITEMS {
            itens[i] = Item::vazio();
            i += 1;
        }
        Self { itens, count: 0, relogio: 0 }
    }

    // Gravacao real de item no historico (desloca nao-fixados para a direita)
    fn gravar(&mut self, kind: Kind, conteudo: &str) {
        self.relogio += 1;
        let b = conteudo.as_bytes();
        let len = b.len().min(MAX_LEN - 1);
        let mut novo = Item::vazio();
        novo.kind = kind;
        novo.dados[..len].copy_from_slice(&b[..len]);
        novo.len = len;
        novo.timestamp = self.relogio;
        let mut i = self.count;
        while i > 0 {
            if !self.itens[i - 1].fixado && i < MAX_ITEMS {
                self.itens[i] = self.itens[i - 1];
            }
            i -= 1;
        }
        self.itens[0] = novo;
        if self.count < MAX_ITEMS {
            self.count += 1;
        }
    }

    fn listar(&self) {
        let mut i = 0;
        while i < self.count {
            self.itens[i].mostrar(i);
            i += 1;
        }
    }

    // Limpeza real: remove apenas itens nao fixados, preservando ordem dos fixados
    fn limpar_nao_fixados(&mut self) {
        let mut w = 0usize;
        let mut r = 0usize;
        while r < self.count {
            if self.itens[r].fixado {
                self.itens[w] = self.itens[r];
                w += 1;
            }
            r += 1;
        }
        self.count = w;
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("           hyb-clip - Daemon de Area de Transferencia      ");
    println!("========================================================");
    let mut clip = Clipboard::novo();
    clip.gravar(Kind::Texto, "senha_do_servidor=Hybridos_2026");
    clip.gravar(Kind::Caminho, "C:\\Users\\dev\\projeto\\main.rs");
    clip.gravar(Kind::Texto, "trecho de codigo copiado");
    clip.gravar(Kind::Imagem, "captura_001.png");
    clip.gravar(Kind::Texto, "item importante fixado");
    clip.itens[0].fixado = true;
    println!("[hyb-clip] Historico completo:");
    clip.listar();
    println!("[hyb-clip] Total de itens: {}", clip.count);
    clip.limpar_nao_fixados();
    println!("[hyb-clip] apos limpar nao-fixados:");
    clip.listar();
    println!("[hyb-clip] Daemon finalizado.");
    sys_sched_yield();
    sys_exit(0)
}
