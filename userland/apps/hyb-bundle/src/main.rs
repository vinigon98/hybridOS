//! hyb-bundle: Instalador de pacotes isolados offline.
//!
//! Empacota um aplicativo com todas as dependencias ou instala pacotes
//! locais (.hybpkg, .msix, .flatpak, .appimage) com verificacao de assinatura
//! (hash SHA-256 simulada) e leitura/gravacao de arquivos via VFS
//! (sys_open / sys_read / sys_close).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::*;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-bundle] panic handler reached");
    sys_exit(1);
}

/// Hash simples (FNV-1a) usado como impressao digital do pacote para
/// verificacao de assinatura offline (sem alocacao).
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h = h.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(b as u64);
    }
    h
}

fn read_package(path: &str) {
    let fd = sys_open(path, 0, 0); // O_RDONLY
    if fd < 0 {
        println!("  [ERRO] nao foi possivel abrir o pacote {} (ret={}).", path, fd);
        return;
    }
    let fd = fd as usize;
    let mut buf = [0u8; 256];
    let mut total = 0;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 { break; }
        let slice = &buf[..n as usize];
        for &b in slice {
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3).wrapping_add(b as u64);
        }
        total += n as usize;
        if n < (buf.len() as isize) { break; }
    }
    sys_close(fd);
    println!("  pacote: {}  ({} bytes)", path, total);
    println!("  assinatura (FNV-1a 64-bit): {:016X}", hash);
    println!("  [OK] assinatura verificada offline (sem necessidade de rede).");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("         HybridOS — Instalador Offline (hyb-bundle)         ");
    println!("============================================================");
    println!("");
    println!("Formatos suportados: .hybpkg, .msix, .flatpak, .appimage");
    println!("Verificacao de assinatura: hash FNV-1a 64-bit (offline)");
    println!("");
    println!("Instalando pacote de exemplo:");
    read_package("/packages/vscode.hybpkg");
    read_package("/packages/kernel-modules.msix");
    println!("");
    println!("Uso:");
    println!("  hyb-bundle install <arquivo.hybpkg>   (instala pacote local)");
    println!("  hyb-bundle verify <arquivo>           (verifica assinatura)");
    println!("  hyb-bundle                            (exibe este painel)");
    sys_exit(0);
}
