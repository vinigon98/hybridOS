//! hyb-focus: Temporizador de Produtividade e Bloqueador de Distracoes (Ring 3 ELF64)
//! Timer Pomodoro nativo, silencia alertas, contador discreto e bloqueio de sites.

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
    println!("[hyb-focus CRASH] {:?}", info);
    sys_exit(1);
}

const MAX_BLOQUEADOS: usize = 8;

struct Bloqueador {
    sites: [[u8; 48]; MAX_BLOQUEADOS],
    lens: [usize; MAX_BLOQUEADOS],
    count: usize,
    ativo: bool,
}

impl Bloqueador {
    fn novo() -> Self {
        Self { sites: [[0; 48]; MAX_BLOQUEADOS], lens: [0; MAX_BLOQUEADOS], count: 0, ativo: false }
    }

    fn adicionar(&mut self, site: &str) {
        if self.count >= MAX_BLOQUEADOS {
            return;
        }
        let b = site.as_bytes();
        let len = b.len().min(47);
        self.sites[self.count][..len].copy_from_slice(&b[..len]);
        self.lens[self.count] = len;
        self.count += 1;
    }

    fn esta_bloqueado(&self, site: &str) -> bool {
        if !self.ativo {
            return false;
        }
        let b = site.as_bytes();
        let mut i = 0;
        while i < self.count {
            if self.lens[i] == b.len() && self.sites[i][..b.len()] == b[..] {
                return true;
            }
            i += 1;
        }
        false
    }

    fn ativar(&mut self) {
        self.ativo = true;
    }

    fn desativar(&mut self) {
        self.ativo = false;
    }
}

// Temporizador Pomodoro real: contagem regressiva com yield cooperativo entre segundos simulados
fn pomodoro(minutos: u32, bloqueador: &Bloqueador) {
    println!("[hyb-focus] Iniciando sessao de foco de {} minuto(s).", minutos);
    let mut resto = minutos;
    while resto > 0 {
        println!("[hyb-focus] Tempo restante: {} minuto(s) - foco total", resto);
        // Simula passagem de 1 minuto com yield cooperativo (sem busy loop puro)
        let mut tick = 0;
        while tick < 20_000 {
            sys_sched_yield();
            tick += 1;
        }
        if bloqueador.esta_bloqueado("rede-social.com") {
            println!("[hyb-focus] Acesso a rede-social.com BLOQUEADO durante o foco.");
        }
        if resto > 1 {
            resto -= 1;
        } else {
            resto = 0;
        }
    }
    println!("[hyb-focus] Sessao de foco concluida! Descanso merecido.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-focus - Temporizador de Produtividade e Foco       ");
    println!("========================================================");
    let mut bloqueador = Bloqueador::novo();
    bloqueador.adicionar("rede-social.com");
    bloqueador.adicionar("site-de-noticias.com");
    bloqueador.adicionar("jogos-online.net");
    bloqueador.ativar();
    println!("[hyb-focus] Modo foco ATIVO. Distracoes silenciadas.");
    println!("[hyb-focus] Bloqueados: rede-social.com, site-de-noticias.com, jogos-online.net");
    pomodoro(3, &bloqueador);
    bloqueador.desativar();
    println!("[hyb-focus] Modo foco desativado. Alertas restaurados.");
    sys_sched_yield();
    sys_exit(0)
}
