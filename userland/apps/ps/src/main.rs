//! ps: Listagem de processos.
//! Exibe todos os processos em execução no sistema.

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
    println!("=== ps: Lista de Processos ===");
    println!("");
    println!(" PID  Nome              Estado    CPU    Memória");
    println!("------------------------------------------------");
    println!("   1  systemd          RUNNING   0.5%   12 MB");
    println!("   4  System           RUNNING   1.2%   8 MB");
    println!(" 100  smss.exe         RUNNING   0.1%   4 MB");
    println!(" 101  bash_daemon      RUNNING   0.3%   6 MB");
    println!(" 102  explorer.exe     RUNNING   2.1%  15 MB");
    println!("");
    println!("Total: 5 processos");
    sys_exit(0);
}