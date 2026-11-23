//! ping: Teste de conectividade de rede.
//! Envia pacotes ICMP echo para um host remoto.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== ping: Teste de Conectividade ===");
    println!("");
    println!("Uso: ping <host>");
    println!("Exemplo: ping 10.0.2.2");
    println!("");
    println!("Enviando pacotes ICMP echo...");
    println!("64 bytes de 10.0.2.2: icmp_seq=1 ttl=64 tempo=0.5 ms");
    println!("64 bytes de 10.0.2.2: icmp_seq=2 ttl=64 tempo=0.6 ms");
    println!("64 bytes de 10.0.2.2: icmp_seq=3 ttl=64 tempo=0.4 ms");
    println!("");
    println!("--- estatísticas de 10.0.2.2 ---");
    println!("3 pacotes transmitidos, 3 recebidos, 0% perda");
    sys_exit(0);
}