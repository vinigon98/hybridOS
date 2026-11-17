//! hyb-vault: Cofre de Credenciais e Chaves do Sistema (Ring 3 ELF64)
//! Keyring criptografado (TPM 2.0 simulado): credenciais Git, SSH, tokens API, senhas SMB.

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
    println!("[hyb-vault CRASH] {:?}", info);
    sys_exit(1);
}

const MAX_ENTRIES: usize = 8;
const MAX_VAL: usize = 48;

#[derive(Copy, Clone)]
struct Entrada {
    nome: [u8; 32],
    nome_len: usize,
    valor: [u8; MAX_VAL],
    valor_len: usize,
    tipo: [u8; 16],
    tipo_len: usize,
    criptografado: bool,
}

struct Vault {
    entradas: [Entrada; MAX_ENTRIES],
    count: usize,
    chave: u64,
}

impl Vault {
    fn novo(chave: u64) -> Self {
        let mut entradas = [Entrada {
            nome: [0; 32],
            nome_len: 0,
            valor: [0; MAX_VAL],
            valor_len: 0,
            tipo: [0; 16],
            tipo_len: 0,
            criptografado: false,
        }; MAX_ENTRIES];
        let mut i = 0;
        while i < MAX_ENTRIES {
            entradas[i] = Entrada {
                nome: [0; 32],
                nome_len: 0,
                valor: [0; MAX_VAL],
                valor_len: 0,
                tipo: [0; 16],
                tipo_len: 0,
                criptografado: false,
            };
            i += 1;
        }
        Self { entradas, count: 0, chave }
    }

    // Derivacao de chave simulada (TPM) e cifragem XOR real dos bytes em repouso
    fn cifrar_xor(dados: &mut [u8], chave: u64) {
        let mut k = chave;
        let mut i = 0;
        while i < dados.len() {
            k = k.wrapping_mul(0x5DEECE66D).wrapping_add(0xB);
            dados[i] ^= (k >> 40) as u8;
            i += 1;
        }
    }

    fn armazenar(&mut self, nome: &str, valor: &str, tipo: &str) {
        if self.count >= MAX_ENTRIES {
            return;
        }
        let nb = nome.as_bytes();
        let vb = valor.as_bytes();
        let tb = tipo.as_bytes();
        let nl = nb.len().min(31);
        let vl = vb.len().min(MAX_VAL - 1);
        let tl = tb.len().min(15);
        let mut e = Entrada {
            nome: [0; 32],
            nome_len: 0,
            valor: [0; MAX_VAL],
            valor_len: 0,
            tipo: [0; 16],
            tipo_len: 0,
            criptografado: false,
        };
        e.nome[..nl].copy_from_slice(&nb[..nl]);
        e.nome_len = nl;
        e.valor[..vl].copy_from_slice(&vb[..vl]);
        e.valor_len = vl;
        e.tipo[..tl].copy_from_slice(&tb[..tl]);
        e.tipo_len = tl;
        Self::cifrar_xor(&mut e.valor[..vl], self.chave);
        e.criptografado = true;
        self.entradas[self.count] = e;
        self.count += 1;
    }

    // Recuperacao real: copia o valor cifrado para um buffer e decifra no local
    fn recuperar_para(&self, nome: &str, saida: &mut [u8]) -> usize {
        let nb = nome.as_bytes();
        let mut i = 0;
        while i < self.count {
            let e = &self.entradas[i];
            if e.nome_len == nb.len() && e.nome[..nb.len()] == nb[..] {
                let vl = e.valor_len.min(saida.len());
                let mut j = 0;
                while j < vl {
                    saida[j] = e.valor[j];
                    j += 1;
                }
                Self::cifrar_xor(&mut saida[..vl], self.chave);
                return vl;
            }
            i += 1;
        }
        0
    }

    fn listar(&self) {
        let mut i = 0;
        while i < self.count {
            let e = &self.entradas[i];
            let n = core::str::from_utf8(&e.nome[..e.nome_len]).unwrap_or("");
            let t = core::str::from_utf8(&e.tipo[..e.tipo_len]).unwrap_or("");
            println!("  - {} [{}] criptografado={}", n, t, e.criptografado);
            i += 1;
        }
    }
}

fn mostrar_mascarado(buf: &[u8]) {
    let s = core::str::from_utf8(buf).unwrap_or("");
    let mut i = 0;
    while i < s.len() {
        print!("*");
        i += 1;
    }
    println!();
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("             hyb-vault - Cofre de Credenciais e Chaves      ");
    println!("========================================================");
    let mut cofre = Vault::novo(0xDEADBEEFCAFEBABE);
    cofre.armazenar("git_github", "ghp_abc123def456", "token");
    cofre.armazenar("ssh_id_rsa", "-----BEGIN OPENSSH PRIVATE KEY-----", "chave");
    cofre.armazenar("smb_nas", "senha_do_nas_2026", "senha");
    cofre.armazenar("api_cloud", "sk-live_xyz789", "token");
    println!("[hyb-vault] Entradas armazenadas (valores cifrados em repouso):");
    cofre.listar();
    let mut buf = [0u8; MAX_VAL];
    println!("[hyb-vault] Recuperando 'git_github': ");
    print!("  valor = ");
    let vl = cofre.recuperar_para("git_github", &mut buf);
    mostrar_mascarado(&buf[..vl]);
    println!("[hyb-vault] Recuperando 'ssh_id_rsa': ");
    print!("  valor = ");
    let vl = cofre.recuperar_para("ssh_id_rsa", &mut buf);
    mostrar_mascarado(&buf[..vl]);
    println!("[hyb-vault] Recuperando 'nao_existe': {} bytes", cofre.recuperar_para("nao_existe", &mut buf));
    println!("[hyb-vault] Cofre selado.");
    sys_sched_yield();
    sys_exit(0)
}
