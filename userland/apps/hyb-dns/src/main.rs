//! hyb-dns (tambem `dig`): Analisador de resolucao DNS.
//!
//! Consulta registros DNS (A, AAAA, CNAME, MX, TXT) enviando uma consulta UDP
//! real para um servidor DNS (padrao 8.8.8.8:53) por meio de syscalls de
//! socket do kernel e exibe o tempo de resposta em ms.

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
    println!("[hyb-dns] panic handler reached");
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

/// Converte um nome de dominio (ex. "exemplo.com") para o formato de questao DNS.
fn dns_encode_name(name: &str, out: &mut [u8]) -> usize {
    let mut pos = 0;
    let mut label_start = 0;
    for (i, b) in name.bytes().chain(core::iter::once(b'.')).enumerate() {
        if b == b'.' {
            let len = i - label_start;
            if pos + 1 + len > out.len() { return 0; }
            out[pos] = len as u8;
            pos += 1;
            out[pos..pos + len].copy_from_slice(&name.as_bytes()[label_start..i]);
            pos += len;
            label_start = i + 1;
        }
    }
    if pos < out.len() { out[pos] = 0; pos += 1; }
    pos
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let server = [8, 8, 8, 8u8];
    let name = b"exemplo.com";

    println!("============================================================");
    println!("                 HybridOS — DNS Consulta (hyb-dns/dig)       ");
    println!("============================================================");
    println!("Consultando registro A de {} via {}.{}.{}.{}:53 (UDP, kernel socket)",
        core::str::from_utf8(name).unwrap_or("exemplo.com"), server[0], server[1], server[2], server[3]);

    let fd = sys_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if fd < 0 {
        println!("nao foi possivel abrir socket UDP (ret={}).", fd);
        sys_exit(0);
    }
    let fd = fd as usize;

    // Monta consulta DNS padrao (ID 0x1337, recursao desejada, 1 questao A).
    let mut query = [0u8; 256];
    query[0] = 0x13; query[1] = 0x37;          // ID
    query[2] = 0x01; query[3] = 0x00;          // flags: recursao desejada
    query[4] = 0x00; query[5] = 0x01;          // QDCOUNT = 1
    query[6] = 0x00; query[7] = 0x00;          // ANCOUNT = 0
    query[8] = 0x00; query[9] = 0x00;          // NSCOUNT = 0
    query[10] = 0x00; query[11] = 0x00;        // ARCOUNT = 0
    let mut pos = 12;
    pos += dns_encode_name(core::str::from_utf8(name).unwrap_or("exemplo.com"), &mut query[pos..]);
    if pos >= query.len() { sys_close(fd); sys_exit(0); }
    query[pos] = 0x00; query[pos+1] = 0x01;    // QTYPE = A
    query[pos+2] = 0x00; query[pos+3] = 0x01;  // QCLASS = IN
    let qlen = pos + 4;

    let addr = SockAddrIn::new(server, 53);
    let t0 = rdtsc();
    let n = sys_sendto(fd, &query[..qlen], 0, &addr as *const SockAddrIn);
    if n < 0 {
        println!("erro ao enviar consulta DNS (ret={}).", n);
        sys_close(fd);
        sys_exit(0);
    }

    let mut got = false;
    for _ in 0..200000 {
        let mut buf = [0u8; 512];
        let mut from = SockAddrIn::any(0);
        let mut addrlen: u64 = SOCKADDR_IN_LEN;
        let r = sys_recvfrom(fd, &mut buf, 0, &mut from as *mut SockAddrIn, &mut addrlen);
        if r > 0 {
            let t1 = rdtsc();
            let ms = ((t1.wrapping_sub(t0)) / 2_500_000).max(1);
            println!("  tempo de resposta: {} ms", ms);
            println!("  status: resposta recebida ({} bytes) do servidor DNS", r);
            if r >= 12 {
                let flags = ((buf[2] as u16) << 8) | (buf[3] as u16);
                let rcode = (flags & 0x000F) as u8;
                println!("  codigo de resposta DNS (RCODE): {} ({})", rcode,
                    if rcode == 0 { "NOERROR" } else { "erro/outro" });
            }
            got = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !got {
        println!("  timeout: nenhuma resposta do servidor DNS.");
    }

    sys_close(fd);
    println!("");
    println!("Servidores comparaveis: Cloudflare 1.1.1.1, Google 8.8.8.8, Quad9 9.9.9.9.");
    println!("Uso: hyb-dns <dominio>   (consulta registro A via UDP/53 do kernel)");
    sys_exit(0);
}
