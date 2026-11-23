//! hyb-cert-user: Gerenciador de certificados de usuário.

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
    println!("=== hyb-cert-user: Certificados de Usuário ===");
    println!("");
    println!("Certificados pessoais armazenados:");
    println!("  [ ] Nenhum certificado instalado");
    println!("");
    println!("Operações disponíveis:");
    println!("  - Importar certificado (.cer / .pfx)");
    println!("  - Exportar certificado");
    println!("  - Ver detalhes do certificado");
    println!("  - Remover certificado");
    println!("");
    println!("Local de armazenamento: /etc/certs/user/");
    sys_exit(0);
}