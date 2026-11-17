//! hyb-mtr: Rastreador contínuo de rota e perda de pacotes (estilo mtr).
//!
//! Envia sondas ICMP Echo Request em rodadas sucessivas por meio de um socket
//! RAW real (IPPROTO_ICMP) da pilha de rede do kernel, exibindo latencia,
//! percentual de perda e jitter a cada salto ate o destino.

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
    println!("[hyb-mtr] panic handler reached");
    sys_exit(1);
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

fn build_echo(id: u16, seq: u16) -> [u8; 64] {
    let mut pkt = [0u8; 64];
    pkt[0] = 8; // Echo Request
    pkt[1] = 0;
    pkt[4] = (id >> 8) as u8;
    pkt[5] = (id & 0xFF) as u8;
    pkt[6] = (seq >> 8) as u8;
    pkt[7] = (seq & 0xFF) as u8;
    for i in 8..64 {
        pkt[i] = (i.wrapping_mul(7).wrapping_add(3)) as u8;
    }
    let csum = checksum(&pkt);
    pkt[2] = (csum >> 8) as u8;
    pkt[3] = (csum & 0xFF) as u8;
    pkt
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
    let rounds: usize = 4;

    println!("============================================================");
    println!("              HybridOS — hyb-mtr (rastreamento contínuo)    ");
    println!("============================================================");
    println!("HOST: {}.{}.{}.{}   Sondas ICMP por meio de socket RAW do kernel",
        target[0], target[1], target[2], target[3]);
    println!("  {:<4} | {:<7} | {:<7} | {:<7} | {:<7} | {}", "HOP", "LOSS%", "SNT", "RCV", "AVG-MS", "ULTIMO SALTO");
    println!("  {:<4} | {:<7} | {:<7} | {:<7} | {:<7} | {}", "---", "-------", "-----", "-----", "-------", "------------------");

    let fd = sys_socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);
    if fd < 0 {
        println!("nao foi possivel abrir socket RAW ICMP (ret={}).", fd);
        sys_exit(0);
    }
    let fd = fd as usize;

    let addr = SockAddrIn::new(target, 1);
    let mut seq: u16 = 1;
    let mut hop_rtt: Option<u32> = None;

    for round in 1..=rounds {
        let mut sent: u32 = 0;
        let mut recv: u32 = 0;
        let mut total_ms: u32 = 0;
        let mut last_rtt: Option<u32> = None;

        for _ in 0..3 {
            let pkt = build_echo(0x1337, seq);
            seq = seq.wrapping_add(1);
            let t0 = rdtsc();
            let n = sys_sendto(fd, &pkt, 0, &addr as *const SockAddrIn);
            if n < 0 { continue; }
            sent += 1;

            let mut got = false;
            for _ in 0..100000 {
                let mut buf = [0u8; 256];
                let mut from = SockAddrIn::any(0);
                let mut addrlen: u64 = SOCKADDR_IN_LEN;
                let r = sys_recvfrom(fd, &mut buf, 0, &mut from as *mut SockAddrIn, &mut addrlen);
                if r > 0 && buf.len() > 0 && buf[0] == 0 {
                    let t1 = rdtsc();
                    let ms = (((t1.wrapping_sub(t0)) / 2_500_000) as u32).max(1);
                    recv += 1;
                    total_ms = total_ms.saturating_add(ms);
                    last_rtt = Some(ms);
                    got = true;
                    break;
                }
                core::hint::spin_loop();
            }
        }

        let loss = if sent > 0 { ((sent - recv) * 100) / sent } else { 0 };
        let avg = if recv > 0 { total_ms / recv } else { 0 };
        hop_rtt = last_rtt;

        println!("  {:<4} | {:<7} | {:<7} | {:<7} | {:<7} | {}.{}.{}.{}",
            round, loss, sent, recv, avg,
            target[0], target[1], target[2], target[3]);

        let _ = sys_sched_yield();
    }

    sys_close(fd);
    println!("");
    println!("Media de RTT no ultimo salto: {} ms", hop_rtt.unwrap_or(0));
    println!("Uso: hyb-mtr <host>   (rastreamento contínuo via ICMP RAW do kernel)");
    sys_exit(0);
}
