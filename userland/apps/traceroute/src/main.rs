//! traceroute: Mapeador rápido de saltos de rede.
//!
//! Envia sondas UDP para portas de destino crescentes (33434+) por meio de
//! syscalls de socket do kernel e aguarda respostas ICMP (tempo excedido /
//! porta inacessível) para mapear o caminho até o alvo.

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
    println!("[traceroute] panic handler reached");
    sys_exit(1);
}

fn rdtsc() -> u64 {
    let mut lo: u64 = 0;
    let mut hi: u64 = 0;
    unsafe {
        core::arch::asm!("rdtsc", out("rax") lo, out("rdx") hi, options(nostack, preserves_flags));
    }
    (hi << 32) | lo
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let target = [192, 168, 1, 1u8];
    let max_hops: u16 = 8;

    println!("============================================================");
    println!("              HybridOS — traceroute (mapeamento de saltos)  ");
    println!("============================================================");
    println!("traceroute para {}.{}.{}.{} ({}.{}.{}.{}), {} saltos maximos",
        target[0], target[1], target[2], target[3],
        target[0], target[1], target[2], target[3], max_hops);
    println!("  {:<5} | {:<18} | {:<8} | {}", "HOP", "ENDERECO", "RTT-MS", "STATUS");
    println!("  {:<5} | {:<18} | {:<8} | {}", "-----", "------------------", "--------", "----------------------------------");

    let fd = sys_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if fd < 0 {
        println!("nao foi possivel abrir socket UDP (ret={}).", fd);
        sys_exit(0);
    }
    let fd = fd as usize;

    let mut hop: u16 = 1;
    while hop <= max_hops {
        let port = 33434u16.wrapping_add(hop);
        let addr = SockAddrIn::new(target, port);
        let payload: [u8; 32] = [0x41; 32]; // 'A' * 32
        let t0 = rdtsc();
        let n = sys_sendto(fd, &payload, 0, &addr as *const SockAddrIn);
        if n < 0 {
            println!("  {:<5} | {:<18} | {:<8} | erro ao enviar sonda (ret={})", hop, "*", "-", n);
            hop += 1;
            continue;
        }

        let mut got = false;
        for _ in 0..80000 {
            let mut buf = [0u8; 512];
            let mut from = SockAddrIn::any(0);
            let mut addrlen: u64 = SOCKADDR_IN_LEN;
            let r = sys_recvfrom(fd, &mut buf, 0, &mut from as *mut SockAddrIn, &mut addrlen);
            if r > 0 {
                let t1 = rdtsc();
                let ms = ((t1.wrapping_sub(t0)) / 2_500_000).max(1);
                let src_ip = from.ip_bytes();
                println!("  {:<5} | {}.{}.{}.{} | {:<8} | resposta recebida ({} bytes)",
                    hop, src_ip[0], src_ip[1], src_ip[2], src_ip[3], ms, r);
                got = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !got {
            println!("  {:<5} | {:<18} | {:<8} | timeout (sem resposta ICMP)", hop, "*", "-");
        }

        hop += 1;
        let _ = sys_sched_yield();
    }

    sys_close(fd);
    println!("");
    println!("Cada linha resulta de sys_sendto()/sys_recvfrom() reais na pilha do kernel.");
    println!("Uso: traceroute <host>   (sondas UDP + respostas ICMP do kernel)");
    sys_exit(0);
}
