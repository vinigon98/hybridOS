//! libhybrid_sys: Dual-ABI Ring 3 System Call SDK for HybridOS.
//!
//! Provides bare-metal primitives for:
//! 1. Linux POSIX (System V AMD64 ABI `syscall`)
//! 2. Windows NT Native API (Microsoft x64 ABI `syscall`)
//! 3. Real Ring 3 inspection of TEB, PEB, and KUSER_SHARED_DATA
//! 4. User-space Heap Allocator and formatted I/O

#![no_std]

use core::ffi::c_void;

pub mod linux;
pub mod net;
pub mod windows;
pub mod nt_env;
pub mod allocator;
pub mod io;

pub use allocator::HybridUserAllocator;
pub use nt_env::*;
pub use net::*;

// ----------------------------------------------------------------------------
// Compiler Built-in Intrinsics for Standalone Ring 3 Execution
// ----------------------------------------------------------------------------

#[allow(non_upper_case_globals)]
#[no_mangle]
pub static _fltused: i32 = 0;

#[no_mangle]
pub extern "C" fn __CxxFrameHandler3() -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d = dest as *mut u8;
    let s = src as *const u8;
    let mut i = 0;
    while i < n {
        *d.add(i) = *s.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dest as *mut u8;
    let mut i = 0;
    while i < n {
        *d.add(i) = c as u8;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d = dest as *mut u8;
    let s = src as *const u8;
    if (d as usize) < (s as usize) {
        memcpy(dest, src, n)
    } else {
        let mut i = n;
        while i > 0 {
            i -= 1;
            *d.add(i) = *s.add(i);
        }
        dest
    }
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let a_ptr = s1 as *const u8;
    let b_ptr = s2 as *const u8;
    let mut i = 0;
    while i < n {
        let a = *a_ptr.add(i);
        let b = *b_ptr.add(i);
        if a != b {
            return (a as i32) - (b as i32);
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn wcslen(s: *const u16) -> usize {
    let mut len = 0;
    while *s.add(len) != 0 {
        len += 1;
    }
    len
}

// ----------------------------------------------------------------------------
// Panic handler (obrigatorio para crates #![no_std] sem a STD).
// Com panic=abort este handler e' um ponto de seguranca; em nenhuma rota
// normal ele deve ser atingido.
// ----------------------------------------------------------------------------
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
