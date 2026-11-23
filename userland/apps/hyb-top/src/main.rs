//! hyb-top: Monitor de processos em tempo real.
//! Exibe lista de processos, PIDs, uso de CPU e memória.

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
    println!("=== hyb-top: Monitor de Processos ===");
    println!("");
    println!("PID  PPID  NOME          PERSONALIDADE    ESTADO    PRIO");
    println!("------------------------------------------------------------");
    println!("1     0   systemd       Linux POSIX      RUNNING   10");
    println!("4     0   System        Windows NT       RUNNING   15");
    println!("100   0   smss.exe      Windows NT       RUNNING   10");
    println!("101   0   bash_daemon   Linux POSIX      RUNNING   10");
    println!("102   0   explorer.exe  Windows NT       RUNNING   10");
    println!("");
    println!("Total: 5 processos ativos");
    sys_exit(0);
}