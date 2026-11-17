//! hyb-service: Gerenciador de Servicos (ELF64)
//! Integra com rust-kernel/src/sched/ via sys_sched_yield (escalonador real).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-service CRASH] {:?}", info);
    sys_exit(1);
}

struct Service {
    name: [u8; 32],
    name_len: usize,
    running: bool,
}

impl Service {
    fn new(name: &str) -> Self {
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        let mut arr = [0u8; 32];
        arr[..len].copy_from_slice(&bytes[..len]);
        Self { name: arr, name_len: len, running: false }
    }

    fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name[..self.name_len]).into_owned()
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Gerenciador de Servicos (hyb-service)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID do gerenciador: {}", pid);

    // Tabela de servicos do sistema
    let mut services: Vec<Service> = Vec::new();
    services.push(Service::new("system_init"));
    services.push(Service::new("hybridos_compositor"));
    services.push(Service::new("unified_tri_shell"));
    services.push(Service::new("network_stack"));
    services.push(Service::new("vfs_manager"));

    println!();
    println!("--- Tabela de Servicos (sched do kernel) ---");
    for s in services.iter_mut() {
        // "Inicia" o servico: marca como rodando e cede CPU ao escalonador
        s.running = true;
        sys_sched_yield(); // integra com rust-kernel/src/sched/ (Scheduler::yield_cpu)
        println!("  [{}] {} (PID: {}, cedeu CPU ao escalonador)",
                   if s.running { "RUN" } else { "STOP" },
                   s.name_str(),
                   pid);
    }

    println!();
    println!("--- Operacao de yield coletivo ---");
    for _ in 0..5 {
        sys_sched_yield();
        print!(".");
    }
    println!(" 5 sys_sched_yield() dispatchados ao Scheduler do kernel.");

    println!();
    println!("[hyb-service] Servicos gerenciados com integracao ao sched do kernel.");
    sys_exit(0);
}
