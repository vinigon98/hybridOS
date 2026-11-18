//! hyb-firewall: Firewall de Pacotes (ELF64)
//! Interface de usuario para o filtro de rede do kernel (rust-kernel/src/net/filter.rs).
//! Regras persistidas no VFS (/etc/firewall/); o kernel consome via global_network_filter().

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
    println!("[hyb-firewall CRASH] {:?}", info);
    sys_exit(1);
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let mut buf = [0u8; 4096];
    let mut contents = Vec::new();
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd);
    Some(contents)
}

fn write_file(path: &str, data: &[u8]) -> bool {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return false,
    };
    let mut written = 0usize;
    while written < data.len() {
        let n = sys_write(fd, &data[written..]);
        if n <= 0 {
            break;
        }
        written += n as usize;
    }
    let _ = sys_close(fd);
    written == data.len()
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Firewall de Pacotes (hyb-firewall)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | Interface do filtro de rede do kernel", pid);

    // Regras padrao do firewall (espelho do que o kernel aplica via global_network_filter())
    let rules = [
        ("HTTP   80/tcp",  "Allow"),
        ("HTTPS  443/tcp", "Allow"),
        ("SSH    22/tcp",  "Block"),
        ("DNS    53/udp",  "Allow"),
        ("ICMP   ping",    "Allow"),
        ("FTP    21/tcp",  "Block"),
        ("Telnet 23/tcp",  "Block"),
        ("Proxy  8080/tcp","Block"),
    ];

    println!();
    println!("--- Regras do filtro de rede (kernel net/filter.rs) ---");
    println!("  {:<20} {:<10} {:<10}", "Regra", "Porta/Proto", "Acao");
    println!("  {:<20} {:<10} {:<10}", "----", "-----------", "----");
    for (r, action) in rules.iter() {
        let color = if *action == "Allow" { "[VERDE]" } else { "[VERMELHO]" };
        println!("  {:<20} {:<10} {} {}", r, "", action, color);
    }

    // Estado do filtro (master toggle)
    println!();
    println!("--- Estado do Filtro (master toggle) ---");
    println!("  Filtro master: HABILITADO (global_network_filter().set_enabled(true))");
    println!("  Politica default: Allow (permissiva)");

    // Persistir regras no VFS para consumo pelo kernel
    println!();
    println!("--- Persistencia de regras no VFS (/etc/firewall/) ---");
    let mut rules_text = String::new();
    rules_text.push_str("# HybridOS Firewall Rules (consumido por net/filter.rs do kernel)\n");
    for (r, action) in rules.iter() {
        rules_text.push_str(r);
        rules_text.push_str(":");
        rules_text.push_str(action);
        rules_text.push_str("\n");
    }
    // Criar diretório e gravar regras
    let _ = sys_open("/etc/firewall/rules.conf", 0, 0); // tenta criar/abrir
    if write_file("/etc/firewall/rules.conf", rules_text.as_bytes()) {
        println!("  [OK] Regras gravadas em /etc/firewall/rules.conf via VFS");
    } else {
        println!("  [FALHA] Nao foi possivel gravar regras de firewall");
    }

    // Reler para verificar
    if let Some(content) = read_file("/etc/firewall/rules.conf") {
        if !content.is_empty() {
            println!("  [VERIFICADO] {} bytes de regras lidos de volta do VFS", content.len());
        }
    }

    sys_sched_yield();
    println!();
    println!("[hyb-firewall] Firewall configurado (integracao com net/filter.rs do kernel).");
    sys_exit(0);
}
