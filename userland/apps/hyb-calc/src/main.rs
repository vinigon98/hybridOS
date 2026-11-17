//! hyb-calc: Calculadora Tecnica e Conversor Bitwise (Ring 3 ELF64)
//! Modos Programador (registradores 8/16/32/64 bits, shift, XOR/AND/OR) e Cientifico.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-calc CRASH] {:?}", info);
    sys_exit(1);
}

// Visoes de registrador
fn reg8(v: u64) -> u8 {
    v as u8
}
fn reg16(v: u64) -> u16 {
    v as u16
}
fn reg32(v: u64) -> u32 {
    v as u32
}
fn reg64(v: u64) -> u64 {
    v
}

// Operacoes bitwise reais
fn bitwise(a: u64, b: u64) -> (u64, u64, u64, u64) {
    (a & b, a | b, a ^ b, !a)
}

// Shifts binarios reais
fn shifts(a: u64, s: u64) -> (u64, u64, u64) {
    (a.wrapping_shl(s as u32), a.wrapping_shr(s as u32), a.rotate_left(s as u32))
}

// MDC real (algoritmo de Euclides)
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

// Fatorial real (precisao simulada em u64 com saturacao)
fn factorial(mut n: u64) -> u64 {
    let mut r: u64 = 1;
    let mut i = 1u64;
    while i <= n {
        r = r.wrapping_mul(i);
        i += 1;
    }
    r
}

// Imprime u64 em binario, octal, decimal e hexadecimal
fn print_bases(v: u64, label: &str) {
    println!("[hyb-calc] {} = 0b{:b} | 0o{:o} | 0x{:X} | {}", label, v, v, v, v);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("========================================================");
    println!("        hyb-calc - Calculadora Tecnica e Conversor       ");
    println!("========================================================");

    let a: u64 = 0xDEADBEEFCAFEBABE;
    let b: u64 = 0x0123456789ABCDEF;
    print_bases(a, "A");
    print_bases(b, "B");

    let (andd, oor, xoor, nott) = bitwise(a, b);
    println!("[hyb-calc] A & B = 0x{:016X}", andd);
    println!("[hyb-calc] A | B = 0x{:016X}", oor);
    println!("[hyb-calc] A ^ B = 0x{:016X}", xoor);
    println!("[hyb-calc] !A    = 0x{:016X}", nott);

    let (shl, shr, rot) = shifts(a, 13);
    println!("[hyb-calc] A << 13 = 0x{:016X}", shl);
    println!("[hyb-calc] A >> 13 = 0x{:016X}", shr);
    println!("[hyb-calc] rotl 13 = 0x{:016X}", rot);

    println!("[hyb-calc] reg8(A)  = 0x{:02X}", reg8(a));
    println!("[hyb-calc] reg16(A) = 0x{:04X}", reg16(a));
    println!("[hyb-calc] reg32(A) = 0x{:08X}", reg32(a));
    println!("[hyb-calc] reg64(A) = 0x{:016X}", reg64(a));

    println!("[hyb-calc] gcd(0xDEADBEEF, 0xCAFEBABE) = 0x{:X}", gcd(0xDEADBEEF, 0xCAFEBABE));
    println!("[hyb-calc] factorial(12) = {}", factorial(12));
    println!("[hyb-calc] Sessao de calculo encerrada.");
    sys_sched_yield();
    sys_exit(0)
}
