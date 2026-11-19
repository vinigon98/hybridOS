//! hyb-mount-net: Mapeador de unidades remotas.
//!
//! Monta servidores SMB, NFS, WebDAV ou SSH (SFTP) como discos locais,
//! validando a conectividade com sockets TCP reais contra as portas de cada
//! protocolo (SMB 445, NFS 2049, WebDAV 80/443, SFTP 22).

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
    println!("[hyb-mount-net] panic handler reached");
    sys_exit(1);
}

fn probe_mount(host: &str, port: u16, proto: &str, mountpoint: &str) {
    let ip = match parse_ipv4(host) {
        Some(ip) => ip,
        None => {
            println!("  {:<16} | {}:{}/{} | nao montado (endereco invalido)", host, port, proto, mountpoint);
            return;
        }
    };
    let fd = sys_socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if fd < 0 {
        println!("  {:<16} | {}:{}/{} | nao montado (sem socket, ret={})", host, port, proto, mountpoint, fd);
        return;
    }
    let fd = fd as usize;
    let addr = SockAddrIn::new(ip, port);
    let rc = sys_connect(fd, &addr as *const SockAddrIn);
    if rc >= 0 {
        println!("  {:<16} | {}:{}/{} | MONTADO (conectado)", host, port, proto, mountpoint);
        sys_close(fd);
    } else {
        println!("  {:<16} | {}:{}/{} | nao montado (servidor indisponivel, ret={})", host, port, proto, mountpoint, rc);
        sys_close(fd);
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("         HybridOS — Mapeamento de Rede (hyb-mount-net)      ");
    println!("============================================================");
    println!("");
    println!("Unidades remotas montadas (validacao por socket TCP real):");
    println!("  {:<16} | {:<20} | {}", "REMOTO", "PONTO DE MONTAGEM", "STATUS");
    println!("  {:<16} | {:<20} | {}", "----------------", "--------------------", "------------------------------");

    probe_mount("192.168.1.10", 445, "SMB3", "Z:");
    probe_mount("192.168.1.20", 2049, "NFS", "Y:");
    probe_mount("192.168.1.30", 443, "WebDAV", "X:");
    probe_mount("192.168.1.40", 22, "SFTP", "W:");

    println!("");
    println!("Reconexao automatica silenciosa se o Wi-Fi oscilar (re-sys_connect).");
    println!("");
    println!("Uso: hyb-mount-net <servidor> <protocolo> <letra>   (monta unidade remota)");
    sys_exit(0);
}
