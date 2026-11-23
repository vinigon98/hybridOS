//! hyb-cert-sys: Gerenciador de certificados do sistema.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-cert-sys: Certificados do Sistema ===");
    println!("");
    println!("Certificados de sistema (CA raiz):");
    println!("  [ ] Nenhum certificado de sistema instalado");
    println!("");
    println!("Operações disponíveis:");
    println!("  - Listar CAs raiz confiáveis");
    println!("  - Importar certificado de sistema");
    println!("  - Revogar certificado");
    println!("  - Ver cadeia de confiança");
    println!("");
    println!("Local de armazenamento: /etc/certs/system/");
    sys_exit(0);
}