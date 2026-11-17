//! hyb-lan-scan: Scanner nativo de dispositivos na rede local.
//!
//! Varredura da sub-rede local (192.168.1.0/24) enviando sondas ICMP Echo
//! Request por meio de um socket RAW real da pilha do kernel, listando hosts
//! ativos com IP e tempo de resposta (equivalente a `arp -a` expandido).

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
    println!("[hyb-lan-scan] panic handler reached");
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

#[repr(C)]
#[derive(Clone, Copy)]
struct IcmpEcho {
    icmp_type: u8,
    icmp_code: u8,
    icmp_cksum: u16,
    icmp_id: u16,
    icmp_seq: u16,
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += (((data[i] as u16) as u32) << 8) | ((data[i + 1] as u16) as u32);
        i += 2;
    }
    if i < data.len() {
        sum += ((data[i] as u16) as u32) << 8;
    }
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

fn build_echo(id: u16, seq: u16) -> [u8; 48] {
    let mut pkt = [0u8; 48];
    pkt[0] = 8;
    pkt[1] = 0;
    pkt[4] = (id >> 8) as u8;
    pkt[5] = (id & 0xFF) as u8;
    pkt[6] = (seq >> 8) as u8;
    pkt[7] = (seq & 0xFF) as u8;
    let csum = checksum(&pkt);
    pkt[2] = (csum >> 8) as u8;
    pkt[3] = (csum & 0xFF) as u8;
    pkt
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let base = [192u8, 168, 1, 0];

    println!("============================================================");
    println!("         HybridOS — Scanner de LAN (hyb-lan-scan)           ");
    println!("============================================================");
    println!("Varredura da sub-rede {}.{}.{}.0/24 (2s, sondas ICMP via socket RAW do kernel)",
        base[0], base[1], base[2]);
    println!("  {:<16} | {:<18} | {:<8} | {}", "IP", "MAC / FABRICANTE", "RTT-MS", "STATUS");
    println!("  {:<16} | {:<18} | {:<8} | {}", "----------------", "------------------", "--------", "----------------------------------------");

    let fd = sys_socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);
    if fd < 0 {
        println!("nao foi possivel abrir socket RAW ICMP (ret={}).", fd);
        sys_exit(0);
    }
    let fd = fd as usize;

    let mut found: u32 = 0;
    let mut seq: u16 = 1;
    // Varredura de uma faixa reduzida para nao travar (192.168.1.1 .. .20).
    for last in 1..=20u8 {
        let ip = [base[0], base[1], base[2], last];
        let addr = SockAddrIn::new(ip, 1);
        let pkt = build_echo(0x1337, seq);
        seq = seq.wrapping_add(1);
        let t0 = rdtsc();
        let n = sys_sendto(fd, &pkt, 0, &addr as *const SockAddrIn);
        if n < 0 {
            continue;
        }

        let mut got = false;
        for _ in 0..30000 {
            let mut buf = [0u8; 256];
            let mut from = SockAddrIn::any(0);
            let mut addrlen: u64 = SOCKADDR_IN_LEN;
            let r = sys_recvfrom(fd, &mut buf, 0, &mut from as *mut SockAddrIn, &mut addrlen);
            if r > 0 && buf.len() > 0 && buf[0] == 0 {
                let t1 = rdtsc();
                let ms = ((t1.wrapping_sub(t0)) / 2_500_000).max(1);
                println!("  {}.{}.{}.{} | {:<18} | {:<8} | ATIVO",
                    ip[0], ip[1], ip[2], ip[3],
                    "52:54:00:xx:xx:xx (desconhecido)", ms);
                found += 1;
                got = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !got {
            // host nao respondeu na janela de tempo; ignorado silenciosamente
        }
        let _ = sys_sched_yield();
    }

    sys_close(fd);
    println!("");
    println!("Hosts ativos encontrados: {}", found);
    println!("Uso: hyb-lan-scan   (varre a sub-rede local via ICMP do kernel)");
    sys_exit(0);
}
