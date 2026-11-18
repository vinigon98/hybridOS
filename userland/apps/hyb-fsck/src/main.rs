//! hyb-fsck: Verificador de Sistema de Arquivos (ELF64)
//! Verifica integridade do VFS/fs do kernel checando caminhos críticos.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use libhybrid_sys::allocator::HybridUserAllocator;
use libhybrid_sys::linux::*;
use libhybrid_sys::{print, println};

#[global_allocator]
static ALLOCATOR: HybridUserAllocator = HybridUserAllocator::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[hyb-fsck CRASH] {:?}", info);
    sys_exit(1);
}

fn probe_path(path: &str) -> bool {
    sys_open(path, 0, 0) >= 0
}

fn read_file(path: &str) -> Option<Vec<u8>> {
    let fd = match sys_open(path, 0, 0) {
        r if r >= 0 => r as usize,
        _ => return None,
    };
    let mut buf = [0u8; 4096];
    let mut contents = Vec::new();
    loop {
        let n = sys_read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n as usize]);
    }
    let _ = sys_close(fd);
    Some(contents)
}

struct FsckResult {
    path: [u8; 64],
    path_len: usize,
    ok: bool,
    readable: bool,
    size: usize,
}

impl FsckResult {
    fn new(path: &str) -> Self {
        let bytes = path.as_bytes();
        let len = bytes.len().min(64);
        let mut arr = [0u8; 64];
        arr[..len].copy_from_slice(&bytes[..len]);
        Self { path: arr, path_len: len, ok: false, readable: false, size: 0 }
    }

    fn path_str(&self) -> String {
        String::from_utf8_lossy(&self.path[..self.path_len]).into_owned()
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("============================================================");
    println!("   HybridOS Verificador de Sistema de Arquivos (hyb-fsck)");
    println!("============================================================");

    let pid = sys_getpid();
    println!("PID: {} | fsck do VFS/fs do kernel", pid);

    // Caminhos críticos do sistema de arquivos
    let critical = [
        "/etc/os-release",
        "/bin",
        "/sbin",
        "/dev",
        "/dev/null",
        "/dev/zero",
        "/dev/console",
        "/c/Windows/System32/ntoskrnl.ini",
        "/tmp",
        "/mnt",
    ];

    println!();
    println!("--- Verificacao de integridade do VFS (fs do kernel) ---");
    let mut results: Vec<FsckResult> = Vec::new();
    let mut errors: u32 = 0;
    let mut checked: u32 = 0;

    for p in critical.iter() {
        let mut r = FsckResult::new(p);
        checked += 1;
        if probe_path(p) {
            r.ok = true;
            if let Some(content) = read_file(p) {
                r.readable = true;
                r.size = content.len();
            }
        } else {
            errors += 1;
        }
        results.push(r);
    }

    println!("  {:<38} {:<8} {:<8} {:<8}", "Caminho", "Existe", "Legível", "Tamanho");
    println!("  {:<38} {:<8} {:<8} {:<8}", "------", "------", "-------", "------");
    for r in results.iter() {
        println!("  {:<38} {:<8} {:<8} {:<8}",
                   r.path_str(),
                   if r.ok { "SIM" } else { "NAO" },
                   if r.readable { "SIM" } else { "NAO" },
                   r.size);
    }

    println!();
    println!("  Caminhos verificados: {}", checked);
    println!("  Caminhos com erro:    {}", errors);
    println!("  Resultado: {}", if errors == 0 { "VFS/fs do kernel: INTEGRO" } else { "VFS/fs do kernel: INCONSISTENCIAS DETECTADAS" });

    sys_sched_yield();
    println!();
    println!("[hyb-fsck] Verificacao de sistema de arquivos concluida (VFS + fs do kernel).");
    sys_exit(0);
}
