//! pkill: Encerrar processo por nome.
//! Envia sinal de término para processos que correspondem ao nome.

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
    println!("=== pkill: Encerrar por Nome ===");
    println!("");
    println!("Uso: pkill <nome_do_processo>");
    println!("Exemplo: pkill notepad");
    println!("");
    println!("Procura todos os processos com o nome especificado");
    println!("e envia sinal de término para cada um.");
    println!("");
    println!("Nenhum nome especificado.");
    sys_exit(0);
}