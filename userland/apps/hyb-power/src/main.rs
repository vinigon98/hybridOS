//! hyb-power — HybridOS Power / MSR Inspector (Ring 3 ELF64).
//!
//! Reads REAL x86_64 Model-Specific Registers via the `rdmsr` instruction
//! (IA32_TIME_STAMP_COUNTER, IA32_STAR, IA32_LSTAR, IA32_SYSENTER_CS, IA32_EFER)
//! and kernel CPU topology via `sys_sysinfo` (200).
//!
//! Note: `rdmsr` is a ring-0 instruction; when executed from Ring 3 the CPU
//! raises #GP. The syscall-based portions (`sys_sysinfo`) return real kernel
//! data when the corresponding kernel syscalls are implemented.

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
    println!("[hyb-power] fatal: {:?}", info);
    sys_exit(1);
}

const MSR_IA32_TSC: u32 = 0x00000010;
const MSR_IA32_SYSENTER_CS: u32 = 0x00000174;
const MSR_IA32_SYSENTER_ESP: u32 = 0x00000175;
const MSR_IA32_SYSENTER_EIP: u32 = 0x00000176;
const MSR_IA32_STAR: u32 = 0xC0000081;
const MSR_IA32_LSTAR: u32 = 0xC0000082;
const MSR_IA32_CSTAR: u32 = 0xC0000083;
const MSR_IA32_EFER: u32 = 0xC0000080;

fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        core::arch::asm!("rdmsr",
            in("ecx") msr,
            out("eax") lo, out("edx") hi,
            options(nostack));
    }
    ((hi as u64) << 32) | (lo as u64)
}

fn read_sysinfo() -> Option<SysInfo> {
    let mut info = SysInfo::default();
    let ret = unsafe { sys_sysinfo(&mut info) };
    if ret < 0 { None } else { Some(info) }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("");
    println!("============================================================");
    println!("        hyb-power - HybridOS Power / MSR Inspector          ");
    println!("============================================================");

    // --- MSR inspection (real hardware registers) ---
    println!("[x86_64 MSRs (rdmsr)]");
    macro_rules! msr {
        ($name:expr, $addr:expr) => {{
            let val = rdmsr($addr);
            println!("  {:<24} 0x{:016X}", $name, val);
        }};
    }
    msr!("IA32_TIME_STAMP_COUNTER", MSR_IA32_TSC);
    msr!("IA32_SYSENTER_CS", MSR_IA32_SYSENTER_CS);
    msr!("IA32_SYSENTER_ESP", MSR_IA32_SYSENTER_ESP);
    msr!("IA32_SYSENTER_EIP", MSR_IA32_SYSENTER_EIP);
    msr!("IA32_STAR", MSR_IA32_STAR);
    msr!("IA32_LSTAR", MSR_IA32_LSTAR);
    msr!("IA32_CSTAR", MSR_IA32_CSTAR);
    msr!("IA32_EFER", MSR_IA32_EFER);

    let efer = rdmsr(MSR_IA32_EFER);
    let mut efer_buf = [0u8; 64];
    let mut elen = 0usize;
    macro_rules! eflag {
        ($bit:expr, $name:expr) => {{
            if (efer >> $bit) & 1 == 1 {
                let n = $name.as_bytes();
                for &b in n {
                    if elen < 63 { efer_buf[elen] = b; elen += 1; }
                }
                if elen < 63 { efer_buf[elen] = b' '; elen += 1; }
            }
        }};
    }
    eflag!(0, "SCE");
    eflag!(8, "LME");
    eflag!(11, "NXE");
    eflag!(12, "SVME");
    eflag!(13, "LMSLE");
    eflag!(14, "FSGBASE");
    if elen == 0 { efer_buf[elen] = b'-'; elen += 1; }
    print!("  EFER flags            ");
    let _ = print!("{}", core::str::from_utf8(&efer_buf[..elen]).unwrap_or(""));
    println!("");

    // --- Kernel CPU topology via sys_sysinfo ---
    println!("");
    println!("[kernel CPU topology (sys_sysinfo -> rust-kernel/src/sched/) ]");
    match read_sysinfo() {
        Some(info) => {
            println!("  cpu_count             : {}", info.cpu_count);
            println!("  uptime ticks          : {}", info.uptime_ticks);
            println!("  total ticks           : {}", info.total_ticks);
        }
        None => {
            println!("  sys_sysinfo unavailable on this kernel build");
        }
    }

    println!("");
    println!("[hyb-power] MSR inspection complete.");
    sys_sched_yield();
    sys_exit(0);
}
