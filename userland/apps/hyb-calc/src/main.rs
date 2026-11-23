//! hyb-calc: Calculadora aritmética simples.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::{print, println};
use libhybrid_sys::linux::sys_exit;

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

fn eval_expr(s: &str, pos: &mut usize) -> f64 {
    let mut val = eval_term(s, pos);
    while *pos < s.len() {
        let c = s.as_bytes()[*pos];
        if c == b'+' { *pos += 1; val += eval_term(s, pos); }
        else if c == b'-' { *pos += 1; val -= eval_term(s, pos); }
        else { break; }
    }
    val
}

fn eval_term(s: &str, pos: &mut usize) -> f64 {
    let mut val = eval_factor(s, pos);
    while *pos < s.len() {
        let c = s.as_bytes()[*pos];
        if c == b'*' { *pos += 1; val *= eval_factor(s, pos); }
        else if c == b'/' { *pos += 1; let f = eval_factor(s, pos); if f != 0.0 { val /= f; } }
        else { break; }
    }
    val
}

fn eval_factor(s: &str, pos: &mut usize) -> f64 {
    while *pos < s.len() && s.as_bytes()[*pos] == b' ' { *pos += 1; }
    if *pos < s.len() && s.as_bytes()[*pos] == b'(' {
        *pos += 1;
        let val = eval_expr(s, pos);
        if *pos < s.len() && s.as_bytes()[*pos] == b')' { *pos += 1; }
        val
    } else {
        let mut num = 0.0;
        let mut dec = false;
        let mut div = 10.0;
        while *pos < s.len() {
            let c = s.as_bytes()[*pos] as char;
            if c.is_ascii_digit() {
                if dec { num += (c as u64 - b'0' as u64) as f64 / div; div *= 10.0; }
                else { num = num * 10.0 + (c as u64 - b'0' as u64) as f64; }
                *pos += 1;
            } else if c == '.' { dec = true; *pos += 1; }
            else { break; }
        }
        num
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("=== hyb-calc ===");
    println!("Expressões: 2+3*4, (10-2)/2, etc.");
    print!("Resultado de 2+3*4 = ");
    let s = "2+3*4";
    let mut pos: usize = 0;
    let result = eval_expr(s, &mut pos);
    println!("{}", result);
    sys_exit(0);
}