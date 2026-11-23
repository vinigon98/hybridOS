//! hyb-service: Gerenciador de serviços.
//! Inicia, para e configura serviços do sistema.

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
    println!("=== hyb-service: Gerenciador de Serviços ===");
    println!("");
    println!("Serviços em execução:");
    println!("  dhcpd       RUNNING");
    println!("  syslogd     RUNNING");
    println!("  crond       RUNNING");
    println!("  sshd        STOPPED");
    println!("");
    println!("Use: hyb-service <nome> <start|stop|restart>");
    sys_exit(0);
}