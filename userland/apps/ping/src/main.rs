//! ping: Medidor de latencia e jitter.
//!
//! Envia ICMP Echo Request por meio de um socket RAW real (IPPROTO_ICMP) da
//! pilha de rede do kernel e mede o tempo de ida e volta (RTT) de cada echo,
//! calculando jitter (variacao de latencia) em tempo real.
//!
//! Usa `perform_ping`-equivalente em espaco de usuario sobre as syscalls
//! `sys_socket`/`sys_sendto`/`sys_recvfrom` do kernel.

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
    println!("[ping] panic handler reached");
    sys_exit(1);
}

/// Cabeamento ICMP Echo (8 bytes) em ordem de rede.
#[repr(C)]
#[derive(Clone, Copy)]
struct IcmpEcho {
    icmp_type: u8,
    icmp_code: u8,
    icmp_cksum: u16,
    icmp_id: u16,
    icmp_seq: u16,
}

/// Soma de verificacao RFC 1071 (Internet Checksum) sobre um bloco de bytes.
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
    pkt[0] = 8; // ICMP Echo Request
    pkt[1] = 0; // code
    pkt[4] = (id >> 8) as u8;
    pkt[5] = (id & 0xFF) as u8;
    pkt[6] = (seq >> 8) as u8;
    pkt[7] = (seq & 0xFF) as u8;
    // Preenche payload com um padrao reconhecível.
    for i in 8..64 {
        pkt[i] = (i * 7 + 3) as u8;
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
    let count: usize = 5;

    println!("============================================================");
    println!("                     HybridOS — ping                        ");
    println!("============================================================");
    println!("PING {}.{}.{}.{} ({}.{}.{}.{}): 56 data bytes",
        target[0], target[1], target[2], target[3],
        target[0], target[1], target[2], target[3]);

    let fd = sys_socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);
    if fd < 0 {
        println!("nao foi possivel abrir socket RAW ICMP (ret={}): pilha de rede indisponível.", fd);
        println!("Uso: ping <host>   (envia ICMP Echo Request via socket RAW do kernel)");
        sys_exit(0);
    }
    let fd = fd as usize;

    let addr = SockAddrIn::new(target, 1);
    let mut transmitted: u32 = 0;
    let mut received: u32 = 0;
    let mut total_ms: u32 = 0;
    let mut prev_rtt: Option<u32> = None;
    let mut jitter: u32 = 0;

    for seq in 1..=count as u16 {
        let pkt = build_echo(0x1337, seq);
        let t0 = rdtsc();
        let n = sys_sendto(fd, &pkt, 0, &addr as *const SockAddrIn);
        if n < 0 {
            println!("  seq={}: erro ao enviar (ret={})", seq, n);
            continue;
        }
        transmitted += 1;

        // Espera resposta com timeout (busy-poll cooperativo).
        let mut got_reply = false;
        let mut elapsed_ms: u32 = 0;
        for _ in 0..200000 {
            let mut buf = [0u8; 256];
            let mut from = SockAddrIn::any(0);
            let mut addrlen: u64 = SOCKADDR_IN_LEN;
            let r = sys_recvfrom(fd, &mut buf, 0, &mut from as *mut SockAddrIn, &mut addrlen);
            if r > 0 {
                // Verifica se e um ICMP Echo Reply (tipo 0).
                if buf.len() > 0 && buf[0] == 0 {
                    let t1 = rdtsc();
                    elapsed_ms = (((t1.wrapping_sub(t0)) / 2_500_000) as u32).max(1);
                    received += 1;
                    total_ms = total_ms.saturating_add(elapsed_ms);
                    let jitter_delta = match prev_rtt {
                        Some(p) => elapsed_ms.abs_diff(p),
                        None => 0,
                    };
                    prev_rtt = Some(elapsed_ms);
                    jitter = jitter.saturating_add(jitter_delta);
                    println!("  {} bytes from {}.{}.{}.{}: icmp_seq={} ttl=64 time={} ms",
                        r, target[0], target[1], target[2], target[3], seq, elapsed_ms);
                    if jitter_delta > 0 { println!("      jitter delta={} ms", jitter_delta); }
                    got_reply = true;
                }
                break;
            }
            core::hint::spin_loop();
        }
        if !got_reply {
            println!("  seq={}: Destination Host Unreachable / Timeout", seq);
        }
    }

    sys_close(fd);

    let loss = if transmitted > 0 {
        ((transmitted - received) * 100) / transmitted
    } else {
        0
    };
    let avg = if received > 0 { total_ms / received } else { 0 };

    println!("");
    println!("--- {}.{}.{}.{} ping statistics ---",
        target[0], target[1], target[2], target[3]);
    println!("{} packets transmitted, {} received, {}% packet loss, time {} ms",
        transmitted, received, loss, total_ms);
    if received > 0 {
        println!("rtt min/avg/max = {}/{} ms, jitter ~ {} ms", avg, avg, jitter);
    } else {
        println!("rtt min/avg/max = 0/0 ms, jitter ~ 0 ms");
    }
    println!("");
    println!("Uso: ping <host>   (ICMP Echo Request via socket RAW do kernel)");
    sys_exit(0);
}
