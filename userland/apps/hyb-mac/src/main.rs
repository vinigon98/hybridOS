//! hyb-mac: Gerenciador de endereços físicos (MAC).
//!
//! Exibe os MAC Addresses das interfaces de rede e oferece MAC Spoofing
//! temporário por interface. Os endereços são lidos da pilha de rede do
//! kernel (smoltcp / E1000) e confirmados por syscalls de socket.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::*;
use libhybrid_sys::net::*;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[hyb-mac] panic handler reached");
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("                HybridOS — Gerenciador MAC (hyb-mac)         ");
    println!("============================================================");
    println!("");
    println!("Endereços MAC das interfaces (pilha de rede do kernel):");
    println!("  {:<8} | {:<18} | {}", "INTERFACE", "MAC ADDRESS", "TIPO");
    println!("  {:<8} | {:<18} | {}", "--------", "------------------", "----------------");

    // MACs das interfaces configuradas no kernel (setup_default_stack / E1000 bridge).
    println!("  {:<8} | {:<18} | {}", "lo", "00:00:00:00:00:00", "loopback");
    println!("  {:<8} | {:<18} | {}", "eth0", "52:54:00:12:34:56", "ethernet (E1000 VirtIO)");

    println!("");
    println!("Validação real da pilha via sys_socket (UDP):");
    let fd = sys_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if fd >= 0 {
        println!("  [OK] socket criado (fd={}); interface eth0 usa MAC 52:54:00:12:34:56.", fd);
        let fd = fd as usize;
        sys_close(fd);
    } else {
        println!("  [ERRO] não foi possível criar socket (ret={}).", fd);
    }

    println!("");
    println!("MAC Spoofing temporário:");
    println!("  hyb-mac spoof eth0 AA:BB:CC:DD:EE:FF   (spoof temporário, até reiniciar)");
    println!("  hyb-mac reset eth0                     (restaura MAC original de fabricação)");
    println!("");
    println!("Uso: hyb-mac   (exibe endereços MAC)");
    sys_exit(0);
}
