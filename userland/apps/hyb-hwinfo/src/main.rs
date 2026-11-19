//! hyb-hwinfo — HybridOS Hardware Information (Ring 3 ELF64).
//!
//! Reports REAL hardware data via the CPUID instruction (vendor ID, brand
//! string, feature flags) and kernel CPU topology via `sys_sysinfo` (200).

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
    println!("[hyb-hwinfo] fatal: {:?}", info);
    sys_exit(1);
}

fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let r = unsafe { core::arch::x86_64::__cpuid_count(leaf, subleaf) };
    (r.eax as u32, r.ebx as u32, r.ecx as u32, r.edx as u32)
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
    println!("          hyb-hwinfo - HybridOS Hardware Information         ");
    println!("============================================================");

    // CPUID function 0: vendor ID + max leaf
    let (eax, ebx, ecx, edx) = cpuid(0, 0);
    let max_leaf = eax;
    let vendor: [u8; 13] = [
        ebx.to_ne_bytes()[0], ebx.to_ne_bytes()[1], ebx.to_ne_bytes()[2], ebx.to_ne_bytes()[3],
        edx.to_ne_bytes()[0], edx.to_ne_bytes()[1], edx.to_ne_bytes()[2], edx.to_ne_bytes()[3],
        ecx.to_ne_bytes()[0], ecx.to_ne_bytes()[1], ecx.to_ne_bytes()[2], ecx.to_ne_bytes()[3],
        0,
    ];
    println!("[cpuid 0] max_leaf = 0x{:08X}", max_leaf);
    println!("[cpuid 0] vendor_id  = {:?}", core::str::from_utf8(&vendor).unwrap_or(""));

    // CPUID function 1: family/model/features
    if max_leaf >= 1 {
        let (a, b, c, d) = cpuid(1, 0);
        let family = ((a >> 8) & 0xF) + ((a >> 20) & 0xFF);
        let model = ((a >> 4) & 0xF) + ((a >> 16) & 0xF);
        let stepping = a & 0xF;
        let sse = (d >> 25) & 1 == 1;
        let sse2 = (d >> 26) & 1 == 1;
        let ht = (d >> 28) & 1 == 1;
        let nx = (d >> 20) & 1 == 1;
        println!("[cpuid 1] family={} model={} stepping={}", family, model, stepping);
        println!("[cpuid 1] features: sse={} sse2={} ht={} nx={}", sse, sse2, ht, nx);
    }

    // Brand string (leaves 0x80000002..0x80000004)
    let (_, _, _, d) = cpuid(0x80000000, 0);
    let brand_max = d;
    let mut brand = [0u8; 49];
    let mut off = 0;
    if brand_max >= 0x80000004 {
        for leaf in 0x80000002u32..=0x80000004u32 {
            let (a, b, c, d) = cpuid(leaf, 0);
            for v in [a, b, c, d] {
                for byte in v.to_ne_bytes() {
                    if off < 48 && byte != 0 {
                        brand[off] = byte;
                    }
                    off += 1;
                }
            }
        }
    }
    brand[48] = 0;
    println!("[brand] {}", core::str::from_utf8(&brand).unwrap_or(""));

    // Kernel CPU topology
    println!("");
    println!("[kernel topology (sys_sysinfo)]");
    match read_sysinfo() {
        Some(info) => {
            println!("  cpu_count   : {}", info.cpu_count);
            println!("  uptime ticks: {}", info.uptime_ticks);
        }
        None => {
            println!("  sys_sysinfo unavailable on this kernel build");
        }
    }

    println!("");
    println!("[hyb-hwinfo] hardware inspection complete (real CPUID data).");
    sys_sched_yield();
    sys_exit(0);
}
