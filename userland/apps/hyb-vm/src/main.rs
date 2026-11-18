#![no_std]
#![no_main]

use libhybrid_sys::linux::{sys_write, sys_exit, sys_sysinfo, STDOUT_FILENO};
use libhybrid_sys::linux::SysInfo;
use core::arch::x86_64::__cpuid;

fn put(s: &str) {
    let _ = sys_write(STDOUT_FILENO, s.as_bytes());
}
fn putln(s: &str) {
    put(s);
    put("\r\n");
}
fn put_u64(v: u64) {
    if v == 0 { put("0"); return; }
    let mut tmp = [0u8; 20];
    let mut n = v;
    let mut pos = 0;
    while n > 0 {
        tmp[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    while pos > 0 {
        pos -= 1;
        put(core::str::from_utf8(&[tmp[pos]]).unwrap_or(""));
    }
}

fn put_u32(v: u32) { put_u64(v as u64); }

// CPUID (x86_64): leaf em eax, retorna eax/ebx/ecx/edx.
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let r = unsafe { __cpuid(leaf) };
    (r.eax, r.ebx, r.ecx, r.edx)
}

fn u32_to_be_bytes(v: u32) -> [u8; 4] {
    let b0 = (v >> 24) as u8;
    let b1 = (v >> 16) as u8;
    let b2 = (v >> 8) as u8;
    let b3 = v as u8;
    [b0, b1, b2, b3]
}

fn run() {
    put("HybridOS hyb-vm v0.1 - Hipervisor Nativo Tipo 1\r\n");
    put("Detecao de ambiente de virtualizacao via CPUID:\r\n");

    let (_a, _b, c, _d) = cpuid(1);
    let hypervisor = (c >> 31) & 1;
    put("  CPUID.1: hypervisor-presente bit = ");
    put_u32(hypervisor);
    put("\r\n");

    let (max_leaf, ebx, ecx, edx) = cpuid(0x40000000);
    put("  CPUID.40000000: max_leaf=0x");
    put_u32(max_leaf);
    put(" assinatura=");
    put(&core::str::from_utf8(&[
        ebx.to_be_bytes()[0], ebx.to_be_bytes()[1], ebx.to_be_bytes()[2], ebx.to_be_bytes()[3],
        edx.to_be_bytes()[0], edx.to_be_bytes()[1], edx.to_be_bytes()[2], edx.to_be_bytes()[3],
        ecx.to_be_bytes()[0], ecx.to_be_bytes()[1], ecx.to_be_bytes()[2], ecx.to_be_bytes()[3],
    ]).unwrap_or(""));
    put("\r\n");

    let mut info = SysInfo::default();
    let rc = sys_sysinfo(&mut info);
    put("  sysinfo rc=");
    put_u64(rc as u64);
    put(" cpu_count=");
    put_u32(info.cpu_count);
    put(" total_mem=");
    put_u64(info.total_mem_bytes);
    put(" uptime_ticks=");
    put_u64(info.uptime_ticks);
    put("\r\n");

    if hypervisor == 1 {
        putln("  => Hipervisor detectado (Tipo 1) sob ambiente virtualizado.");
    } else {
        putln("  => Execucao direta no metal (sem hipervisor CPUID).");
    }
    putln("hyb-vm: conclusao da analise de virtualizacao.");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    run();
    sys_exit(0);
}
